

use std::net::SocketAddr;
use anyhow::{Result, Context, bail};
use bytes::{Buf, BytesMut};
use tracing::info;
use crate::{args::ClientArgs, async_rt::async_tcp::{AsyncTcpStream2}, packet::{HandshakeRequest, self, PacketType, HandshakeResponse, BufPair}};
use super::transfer::{read_specific_packet, xfer_sending, xfer_recving};



pub async fn run_as_client<S>(args: &ClientArgs) -> Result<()> 
where
    S: AsyncTcpStream2<BytesMut>,
    // S: AsyncTcpStream,
    // S: AsyncReadBuf<Buf = BytesMut>,
    // S: AsyncWriteAllBuf<Buf = BytesMut>,
    // S: AsyncWriteBuf<Buf = BytesMut>,
{
    let target_addr = format!("{}:{}", args.host, args.common.server_port);

    let bind_addr: SocketAddr = format!("{}:{}", args.common.bind.as_deref().unwrap_or_else(||"0.0.0.0"), args.cport).parse()?; 

    info!("Connecting to [{}]...", target_addr);
    
    let mut socket = S::async_bind_and_connect(bind_addr, &target_addr).await?;
    
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


