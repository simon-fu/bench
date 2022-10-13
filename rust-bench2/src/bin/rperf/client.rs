

use std::net::SocketAddr;
use anyhow::{Result, Context, bail};
use bytes::Buf;
use super::{packet::{HandshakeRequest, self, PacketType, HandshakeResponse}};
use tracing::info;

use crate::{CommonArgs, transfer::{BufPair, read_specific_packet, xfer_sending, xfer_recving}, async_rt::async_tcp::AsyncTcpStream};

#[derive(Debug, Clone)]
pub struct ClientArgs {
    pub common: CommonArgs,
    pub host: String,
    pub len: usize,
    pub is_reverse: bool,
    pub cport: u16,
    pub secs: u64,
}


// async fn tcp_bind_and_connect<A>(bind_addr: SocketAddr, target_addr: A) -> std::io::Result<TcpStream> 
// where
//     A: ToSocketAddrs
// {
//     let mut connect_result = Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Not found target addr"));
//     for addr in lookup_host(&target_addr).await? {
//         let socket = TcpSocket::new_v4()?;
//         socket.set_reuseaddr(true)?;
//         // socket.set_reuseport(true)?;
//         socket.bind(bind_addr)?;
//         connect_result = socket.connect(addr).await;
//         if connect_result.is_ok() {
//             break;
//         }
//     }
//     connect_result
// }

pub async fn run_as_client<S>(args: &ClientArgs) -> Result<()> 
where
    S: AsyncTcpStream,
{
    // info!("client {:?}", args);
    let target_addr = format!("{}:{}", args.host, args.common.server_port);

    let bind_addr: SocketAddr = format!("{}:{}", args.common.bind.as_deref().unwrap_or_else(||"0.0.0.0"), args.cport).parse()?; 

    info!("Connecting to [{}]...", target_addr);
    
    let mut socket = S::async_bind_and_connect(bind_addr, &target_addr).await?;
    // let mut socket = TcpStream::connect(&target_addr).await?;
    
    info!("local [{}] connected to [{}]", socket.get_local_addr()?, target_addr);

    let mut buf2 = BufPair::default();

    let hreq = HandshakeRequest {
        ver: packet::VERSION,
        is_reverse: args.is_reverse,
        data_len: args.len,
        secs: args.secs,
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

    if !args.is_reverse {
        xfer_sending(&mut socket, &mut buf2, &hreq).await?;
    } else {
        xfer_recving(&mut socket, &mut buf2).await?;
    }

    socket.async_shutdown().await?;

    info!( "Done");

    Ok(())
    
}


