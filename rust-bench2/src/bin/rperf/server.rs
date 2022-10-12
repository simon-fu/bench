
use anyhow::{Result, Context, bail};
use bytes::Buf;
use super::{packet::{self, PacketType, HandshakeRequest, HandshakeResponse, HandshakeResponseCode}};
use tokio::{net::{TcpListener, TcpStream}, io::AsyncWriteExt};
use tracing::info;

use crate::{CommonArgs, common::{read_specific_packet, xfer_recving, BufPair, xfer_sending}};



#[derive(Debug, Clone)]
pub struct ServerArgs {
    pub common: CommonArgs
}

pub async fn run_as_server(args: &ServerArgs) -> Result<()> { 

    let bind = args.common.bind.as_deref().unwrap_or_else(||"0.0.0.0");
    let bind = format!("{}:{}", bind, args.common.server_port);

    let listener = TcpListener::bind(&bind).await
    .with_context(||format!("fail to bind at [{}]", bind))?;

    loop {
        info!("-------------------------------------------------");
        info!("Server listening on {}", listener.local_addr().with_context(||"no local addr")?);
        info!("-------------------------------------------------");

        let (socket, remote) = listener.accept().await.with_context(||"fail to accept")?;
        info!("Accepted connection from {}", remote);

        let mut session = Session {
            socket,
            buf2: BufPair::default(),
        };

        let r = service_session(&mut session).await;
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

struct Session {
    socket: TcpStream,
    buf2: BufPair,
}

async fn service_session(session: &mut Session) -> Result<()> {

    let header = read_specific_packet(&mut session.socket, PacketType::HandshakeRequest, &mut session.buf2.ibuf).await?;

    let data = &session.buf2.ibuf[header.offset..header.offset+header.len];
    let req: HandshakeRequest = serde_json::from_slice(data).with_context(||"invalid handshake request")?;
    session.buf2.ibuf.advance(header.offset+header.len);

    if req.ver != packet::VERSION {
        packet::encode_json(
            PacketType::HandshakeResponse,
            &HandshakeResponse {
                ver: packet::VERSION,
                code: HandshakeResponseCode::VersionNotMatch as u8,
            }, 
            &mut session.buf2.obuf
        )?;
        session.socket.write_all_buf(&mut session.buf2.obuf).await?;
        bail!("expect version [{}] but [{}]", packet::VERSION, req.ver);
    }

    packet::encode_json(
        PacketType::HandshakeResponse,
        &HandshakeResponse {
            ver: packet::VERSION,
            code: HandshakeResponseCode::Success as u8,
        }, 
        &mut session.buf2.obuf
    )?;
    session.socket.write_all_buf(&mut session.buf2.obuf).await?;

    if !req.is_reverse {
        xfer_recving(&mut session.socket, &mut session.buf2).await
    } else {
        xfer_sending(&mut session.socket, &mut session.buf2, req.data_len).await
    }
    

}

