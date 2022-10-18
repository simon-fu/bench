

use std::{net::SocketAddr, sync::{Arc, atomic::{AtomicBool, Ordering}}, collections::HashMap, pin::Pin, task::Poll};
use anyhow::{Result, Context, bail};
use bytes::{Buf, BytesMut};
use futures::{task::AtomicWaker, FutureExt};
use parking_lot::Mutex;
use rust_bench::util::{async_rt::async_tcp::{AsyncTcpStream2, VRuntime}, now_millis, period_call::period_call, traffic::{AtomicTraffic, ToHuman, Traffic, TrafficOp, TrafficRate}, interval::{GetRateState, PeriodRate, CalcRate}, atomic_count::{conn_count::{ConnCount, ConnSnapshot, TOTAL, DONE, ACTIVE}, ToAtomicRateHuman}, pacer::Pacer};
use tracing::{info, debug};
use crate::{args::ClientArgs, packet::{HandshakeRequest, self, PacketType, HandshakeResponse, BufPair}};
use super::transfer::{read_specific_packet, xfer_sending, xfer_recving};



pub async fn run_as_client<RT, S>(args: ClientArgs) -> Result<()> 
where
    RT: VRuntime,
    S: AsyncTcpStream2<BytesMut>,
    // S: AsyncTcpStream,
    // S: AsyncReadBuf<Buf = BytesMut>,
    // S: AsyncWriteAllBuf<Buf = BytesMut>,
    // S: AsyncWriteBuf<Buf = BytesMut>,
{

    let shared = Arc::new(Shared { 
        bind_addr: args.bind.parse()?,
        args,
        state: Default::default(),
        conns: Default::default(),
        traffic: Default::default(),
    });

    let period_job = PeriodRate::new(shared.clone(), |ctx, completed, (delta, rate)| { 
        
        // let rate = rate.unwrap_or_else(||Rate::default()); 

        if delta.traffic.packets() != 0 || completed { 
            if let Some(rate) = &rate {
                info!("traffic: {}", rate.traffic.to_human());
            }
        }
        
        if !delta.conns.is_zero()|| completed { 
            let rate = rate.map(|v|v.conns);
            info!("connections: {}", ctx.conns.to_atomic_rate_human(&delta.conns, &rate));
        }
    });

    let guard = period_call::<RT, _>(period_job);

    let pacer = Pacer::new(shared.args.cps as u64);

    for _ in 0..shared.args.conns { 
        
        let r = pacer.get_sleep_duration(shared.conns.get(TOTAL) as u64);
        if let Some(d) = r {
            RT::async_sleep(d).await;
        }

        let shared = shared.clone();
        
        shared.conns.add_only(TOTAL, 1);

        RT::spawn(async move {
            let r = run_one::<S>(&shared).await;
            if let Err(e) = r {
                info!("connection error [{}]", e);
            }
            shared.conns.add_and_wake(DONE, 1);
        });
    }

    info!("fired conns {}", shared.args.conns);

    let mut watcher = shared.conns.watch();

    // wait for setup completed
    loop {
        watcher.wait_for().await;
        if watcher.last().is_equ2(TOTAL, ACTIVE) {
            break;
        }
        if let Some(n) = watcher.last().get(DONE) {
            if *n > 0 {
                bail!("somthing wrong when waiting fo setup completed")
            }
        }
    }

    info!("all conns setup");
    {
        let mut state = shared.state.lock();
        
        state.stage = Stage::Xfering;
        info!("xfering stage");

        for (_k, ss) in &state.sessions {
            ss.kick();
        }
    }
    

    loop {
        watcher.wait_for().await;
        if watcher.last().is_equ2(TOTAL, DONE) {
            break;
        }
    }

    if let Some(task) = guard.into_task() {
        task.await;
    }

    {
        let mut state = shared.state.lock();
        state.stage = Stage::Done;
    }

    info!( "Done");

    Ok(())
    
}


async fn run_one<S>(shared: &Arc<Shared>) -> Result<()> 
where
    S: AsyncTcpStream2<BytesMut>,
{
    debug!("Connecting to [{}]...", shared.args.target);
    
    let mut socket = S::async_bind_and_connect(shared.bind_addr, &shared.args.target).await?;
    
    debug!("local [{}] connected to [{}]", socket.get_local_addr()?, shared.args.target);

    let mut buf2 = BufPair::default();

    let hreq = HandshakeRequest {
        ver: packet::VERSION,
        is_reverse: shared.args.is_reverse,
        data_len: shared.args.packet_len(),
        secs: shared.args.secs,
        timestamp: now_millis(),
    };

    packet::encode_json(PacketType::HandshakeRequest, &hreq, &mut buf2.obuf)?;
    socket.async_write_all_buf(&mut buf2.obuf).await?;

    let header = read_specific_packet(&mut socket, PacketType::HandshakeResponse, &mut buf2.ibuf).await?;
    let data = &buf2.ibuf[header.offset..header.offset+header.len];
    let rsp: HandshakeResponse = serde_json::from_slice(data).with_context(||"invalid handshake request")?;
    buf2.ibuf.advance(header.offset+header.len);
    if rsp.ver != packet::VERSION {
        bail!("expect version [{}] but [{}]", packet::VERSION, rsp.ver);
    }

    let session = Arc::new(Session::default());

    shared.state.lock().sessions.insert(rsp.cid, session.clone());

    shared.conns.add_and_wake(ACTIVE, 1);

    futures::select! {
        _r = session.as_ref().fuse() =>{}

        r = socket.async_read_buf(&mut buf2.ibuf).fuse() => {
            let n = r?;
            if n == 0 {
                bail!("waiting for kick but disconnected")
            } else {
                bail!("waiting for kick but incoming data")
            }
        }
    }

    if !shared.args.is_reverse {
        xfer_sending(&mut socket, &mut buf2, &hreq, &shared.traffic).await?;
    } else {
        xfer_recving(&mut socket, &mut buf2, &shared.traffic).await?;
    }

    socket.async_shutdown().await?;
    Result::<()>::Ok(())
}


struct Shared {
    args: ClientArgs,
    bind_addr: SocketAddr,
    
    conns: ConnCount,
    traffic: AtomicTraffic, 

    state: Mutex<State>,
}

impl GetRateState for Shared {
    type Output = RateState;
    fn get_rate_state(&self) -> Self::Output { 
        RateState {
            conns: self.conns.snapshot(),
            traffic: self.traffic.get_traffic(),
        }
    }
}

const ORDERING: Ordering = Ordering::Relaxed;

struct RateState {
    conns: ConnSnapshot,
    traffic: Traffic,
}

#[derive(Debug, Clone, Copy, Default)]
struct Rate {
    conns: ConnSnapshot,
    traffic: TrafficRate,
}

impl CalcRate for RateState {
    type Delta = RateState;
    type Rate = Rate;

    fn calc_rate(&self, delta: &Self::Delta, duration: std::time::Duration) -> Self::Rate  {
        let conns = self.conns.calc_rate(&delta.conns, duration);
        Rate {
            conns,
            traffic: self.traffic.calc_rate(&delta.traffic, duration),
        }
    }

    fn calc_delta_only(&self, new_state: &Self) -> Self::Delta  {
        RateState {
            conns: self.conns.calc_delta_only(&new_state.conns),
            traffic: self.traffic.calc_delta_only(&new_state.traffic),
        }
    }
}


#[derive(Default)]
struct State {
    stage: Stage,
    sessions: HashMap< u64, Arc<Session> >,
}

enum Stage {
    Connecting = 1,
    Xfering = 2,
    Done = 3,
}

impl Default for Stage { 
    fn default() -> Self {
        Self::Connecting
    }
}

#[derive(Debug, Default)]
struct Session {
    kicked: AtomicBool,
    waker: AtomicWaker,
}

impl Session { 
    pub fn kick(&self) {
        self.kicked.store(true, ORDERING);
        self.waker.wake();
    }
}


impl std::future::Future for &Session {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<()> {
        if self.kicked.load(ORDERING) {
            return Poll::Ready(());
        }

        self.waker.register(cx.waker());

        if self.kicked.load(ORDERING) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}


