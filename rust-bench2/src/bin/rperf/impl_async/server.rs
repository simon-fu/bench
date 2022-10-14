
use anyhow::{Result, Context, bail};
use bytes::{Buf, BytesMut};
use crate::{args::ServerArgs, async_rt::async_tcp::{AsyncTcpListener2, AsyncTcpStream2}, packet::BufPair};

use super::{super::{packet::{self, PacketType, HandshakeRequest, HandshakeResponse, HandshakeResponseCode}}, transfer::{read_specific_packet, xfer_recving, xfer_sending}};
use tracing::info;





pub async fn run_as_server<L, S>(args: &ServerArgs) -> Result<()> 
where
    L: AsyncTcpListener2<BytesMut>,
{ 

    let bind = args.common.bind.as_deref().unwrap_or_else(||"0.0.0.0");
    let bind = format!("{}:{}", bind, args.common.server_port);
    let bind = bind.parse()?;
    let listener = L::async_listen(bind).await
    .with_context(||format!("fail to bind at [{}]", bind))?;

    let mut num = 0_u64;

    loop {
        num += 1;
        info!("-------------------------------------------------");
        info!("Server listening on {} (test #{})", listener.get_local_addr().with_context(||"no local addr")?, num);
        info!("-------------------------------------------------");

        let (mut socket, remote) = listener.async_accept().await.with_context(||"fail to accept")?;
        info!("Accepted connection from [{}]", remote);

        // let mut session = Session {
        //     socket,
        //     buf2: BufPair::default(),
        // };
        let mut buf2 = BufPair::default();

        let r = service_session(&mut socket, &mut buf2).await;
        match r {
            Ok(_r) => {
            },
            Err(e) => {
                info!("connection error [{:?}]", e);
            },
        }
        info!("the client has terminated");         
    }

}

async fn service_session<S>(socket: &mut S, buf2: &mut BufPair) -> Result<()> 
where
    S: AsyncTcpStream2<BytesMut>,
{

    let header = read_specific_packet(socket, PacketType::HandshakeRequest, &mut buf2.ibuf).await?;

    let data = &buf2.ibuf[header.offset..header.offset+header.len];
    let hreq: HandshakeRequest = serde_json::from_slice(data).with_context(||"invalid handshake request")?;
    buf2.ibuf.advance(header.offset+header.len);

    if hreq.ver != packet::VERSION {
        packet::encode_json(
            PacketType::HandshakeResponse,
            &HandshakeResponse {
                ver: packet::VERSION,
                code: HandshakeResponseCode::VersionNotMatch as u8,
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
        }, 
        &mut buf2.obuf
    )?;
    socket.async_write_all_buf(&mut buf2.obuf).await?;

    if !hreq.is_reverse {
        xfer_recving(socket, buf2).await
    } else {
        xfer_sending(socket, buf2, &hreq).await
    }
    

}

