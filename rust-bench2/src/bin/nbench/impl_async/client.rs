

use std::{net::SocketAddr, sync::Arc};
use anyhow::{Result, Context, bail};
use bytes::{Buf, BytesMut};
use event_listener::Event;
use futures::FutureExt;
use rust_bench::util::{async_rt::async_tcp::{AsyncTcpStream2, VRuntime}, now_millis, pacer::Pacer, histogram::{Histogram, self}};
use tracing::{info, debug};
use crate::{args::ClientArgs, packet::{HandshakeRequest, self, PacketType, HandshakeResponse, BufPair}, impl_async::common::print_latency_percentile};
use super::transfer::{read_specific_packet, xfer_sending, xfer_recving};
use super::conn_stati::{ACTIVE, TOTAL, DONE, period_stati, GetConnsStati, ConnsStati};


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
        bind_addr: match &args.bind {
            Some(bind) => Some(bind.parse()?),
            None => None,
        },
        args,
        event: Default::default(),
        stati: Default::default(),
        hist: histogram::new_latency("client_hist", "client hist help")?,
    });

    
    let guard = period_stati::<RT, _>(shared.clone());

    let pacer = Pacer::new(shared.args.cps as u64);

    for _ in 0..shared.args.conns { 
        
        if shared.stati.conns().get_at(DONE) > 0 {
            bail!("some connections had broken when connecting")
        }

        let r = pacer.get_sleep_duration(shared.stati.conns().get_at(TOTAL) as u64);
        if let Some(d) = r {
            RT::async_sleep(d).await;
        }

        let shared = shared.clone();
        
        shared.stati.conns().add_at(TOTAL, 1);

        RT::spawn(async move {
            let r = run_one::<RT, S>(&shared).await;
            if let Err(e) = r {
                info!("connection error [{}]", e);
            }
            shared.stati.conns().add_at(DONE, 1);
        });
    }
    
    let r = shared.stati.wait_for_conns_setup().await;
    if !r {
        bail!("some connections had broken when waiting for connecting completed")
    }
    info!("all connections setup");

    if !shared.args.is_reverse && shared.args.all_conns() != shared.args.conns {
        info!("press Enter to continue...");
        let _ = std::io::Read::read(&mut std::io::stdin(), &mut [0u8]).unwrap();
    }

    // kick xfer
    shared.event.notify(usize::MAX);

    shared.stati.wati_for_conns_done(guard).await;

    print_latency_percentile(&shared.hist)?;

    info!( "Done");

    Ok(())
    
}


async fn run_one<RT, S>(shared: &Arc<Shared>) -> Result<()> 
where
    RT: VRuntime,
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
        pps: shared.args.pps(),
        conns: shared.args.all_conns(),
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

    let mut hist = shared.hist.local();
    shared.stati.conns().add_at(ACTIVE, 1);


    if !shared.args.is_reverse {
        let listener = shared.event.listen();

        futures::select! {

            _r = listener.fuse() => {}
    
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


    if !shared.args.is_reverse {
        xfer_sending::<RT, _>(&mut socket, &mut buf2, &hreq, shared.stati.traffic()).await?;
    } else {
        xfer_recving(&mut socket, &mut buf2, shared.stati.traffic(), 0, &mut hist).await?;
    }

    socket.async_shutdown().await?;
    Result::<()>::Ok(())
}




impl GetConnsStati for Shared {
    fn get_conns_stati(&self) -> &ConnsStati {
        &self.stati
    }
}

struct Shared {
    args: ClientArgs,
    bind_addr: Option<SocketAddr>,
    event: Event,
    stati: ConnsStati,
    hist: Histogram,
}

