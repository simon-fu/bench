
use std::sync::{Arc, atomic::{AtomicU32, Ordering}};

use anyhow::{Result, Context, bail};
use bytes::{Buf, BytesMut};
use event_listener::Event;
use futures::FutureExt;
use rust_bench::util::{async_rt::async_tcp::{AsyncTcpListener2, AsyncTcpStream2, VRuntime}, now_millis, histogram::{self, LocalHistogram}};
use crate::{args::ServerArgs, packet::BufPair, impl_async::common::print_latency_percentile};

use super::{super::{packet::{self, PacketType, HandshakeRequest, HandshakeResponse, HandshakeResponseCode}}, transfer::{read_specific_packet, xfer_recving, xfer_sending}, conn_stati::{ACTIVE, TOTAL, DONE, period_stati, GetConnsStati, ConnsStati}};

use tracing::{info, debug};





pub async fn run_as_server<RT, L, S>(args: &ServerArgs) -> Result<()> 
where
    RT: VRuntime,
    L: AsyncTcpListener2<BytesMut>,
{ 

    let bind = args.bind.parse()?;
    let listener = L::async_listen(bind).await
    .with_context(||format!("fail to bind at [{}]", bind))?;

    let ctx: Arc<Server> = Default::default();

    let mut num = 0_u64;
    let mut cid = 0_u64;

    loop {
        num += 1;
        info!("-------------------------------------------------");
        info!("Server listening on {} (test #{})", listener.get_local_addr().with_context(||"no local addr")?, num);
        info!("-------------------------------------------------");

        run_one::<RT, L, S>(&listener, &mut cid, &ctx).await?;
    }

}

async fn run_one<RT, L, S>(listener: &L, cid: &mut u64, ctx: &Arc<Server>) -> Result<()> 
where
    RT: VRuntime,
    L: AsyncTcpListener2<BytesMut>,
{    

    ctx.stati.conns().reset_to(0);
    ctx.all_conns.store(0, Ordering::Release);

    let guard = period_stati::<RT, _>(ctx.clone());

    let hist = histogram::new_latency("latency", "latency help")?;
    // let mut watcher = ctx.stati.conns().watch(); 

    loop {

        futures::select! {
            _r = ctx.stati.wait_next_period().fuse() => { 
                let snapshot = ctx.stati.conns().snapshot();
                if snapshot.get_at(TOTAL) > 0 &&  snapshot.is_equ2(TOTAL, DONE) {
                    break;
                }
            }

            // _r = watcher.wait_for().fuse() => {
            //     if watcher.last().is_equ2(TOTAL, DONE) { 
            //         break;
            //     }
            // }

            r = listener.async_accept().fuse() => {
                let (mut socket, remote) = r.with_context(||"fail to accept")?;
                debug!("Accepted connection from [{}]", remote);
        
                ctx.stati.conns().add_at(TOTAL, 1);
                let ctx = ctx.clone();
                let mut hlocal = hist.local();
                let mut buf2 = BufPair::default();
                *cid += 1;
    
                let cid = *cid;
                RT::spawn(async move {
                    let r = service_session::<RT, _>(&mut socket, &mut buf2, cid, &ctx, &mut hlocal).await;
                    match r {
                        Ok(_r) => {
                        },
                        Err(e) => {
                            debug!("connection error [{:?}]", e);
                        },
                    }
                    ctx.stati.conns().add_at(DONE, 1);
                });
            }
        }
                
    }
    
    if let Some(task) = guard.into_task() {
        task.await;
    }

    info!("all clients have terminated"); 
    print_latency_percentile(&hist)?;

    info!(""); 

    Ok(())
}



#[derive(Debug, Default)]
struct Server {
    stati: ConnsStati,
    event: Event,
    all_conns: AtomicU32,
}

impl GetConnsStati for Server {
    fn get_conns_stati(&self) -> &ConnsStati {
        &self.stati
    }
}


async fn service_session<RT, S>(socket: &mut S, buf2: &mut BufPair, cid: u64, ctx: &Arc<Server>, hist: &mut LocalHistogram) -> Result<()> 
where
    RT: VRuntime,
    S: AsyncTcpStream2<BytesMut>,
{

    let header = read_specific_packet(socket, PacketType::HandshakeRequest, &mut buf2.ibuf).await?;

    let data = &buf2.ibuf[header.offset..header.offset+header.len];
    let hreq: HandshakeRequest = serde_json::from_slice(data).with_context(||"invalid handshake request")?;
    buf2.ibuf.advance(header.offset+header.len);
    let timestamp = now_millis();

    if hreq.ver != packet::VERSION {
        packet::encode_json(
            PacketType::HandshakeResponse,
            &HandshakeResponse {
                ver: packet::VERSION,
                code: HandshakeResponseCode::VersionNotMatch as u8,
                timestamp,
                cid,
            }, 
            &mut buf2.obuf
        )?;
        socket.async_write_all_buf(&mut buf2.obuf).await?;
        bail!("expect version [{}] but [{}]", packet::VERSION, hreq.ver);
    }

    packet::encode_json(
        PacketType::HandshakeResponse,
        &HandshakeResponse {
            ver: packet::VERSION,
            code: HandshakeResponseCode::Success as u8,
            timestamp,
            cid,
        }, 
        &mut buf2.obuf
    )?;
    socket.async_write_all_buf(&mut buf2.obuf).await?;

    let delta_ts = timestamp - hreq.timestamp;

    ctx.stati.conns().add_at(ACTIVE, 1);

    if !hreq.is_reverse {
        xfer_recving(socket, buf2, ctx.stati.traffic(), delta_ts, hist).await
    } else {

        wait_for_kick(socket, buf2, ctx, hreq.conns).await?;

        xfer_sending::<RT, _>(socket, buf2, &hreq, ctx.stati.traffic()).await
    }
    

}

async fn wait_for_kick<S>(socket: &mut S, buf2: &mut BufPair, ctx: &Arc<Server>, conns: u32) -> Result<()> 
where
    S: AsyncTcpStream2<BytesMut>,
{
    let r = ctx.all_conns.compare_exchange(0, conns, Ordering::Acquire, Ordering::Relaxed);
    let n = match r {
        Ok(_n) => {
            info!("reverse mode and plan connections [{}]", conns);
            conns
        },
        Err(n) => { 
            n
        },
    };
    if ctx.stati.conns().get_at(ACTIVE) == n as i64 {
        ctx.stati.wait_next_period().await;
        info!("got all connections [{}], kick sending", n);
        ctx.event.notify(usize::MAX);
    } else {
        futures::select! {
            _r = ctx.event.listen().fuse() => {}
            r = socket.async_read_buf(&mut buf2.ibuf).fuse() => {
                let n = r?;
                if n == 0 {
                    bail!("waiting for kick but disconnected")
                } else {
                    bail!("waiting for kick but incoming data")
                }
            }
        }
    }
    Ok(())
}