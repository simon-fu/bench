use std::time::Instant;
use anyhow::{Result, bail};
use bytes::{BytesMut, Buf};
use rust_bench::util::traffic::{TrafficSpeed, Traffic};
use super::packet::{self, PacketType, Header};
use tokio::{net::TcpStream, io::{AsyncWriteExt, AsyncReadExt}};
use tracing::info;


#[derive(Default)]
pub struct BufPair {
    pub ibuf: BytesMut,
    pub obuf: BytesMut,
}


pub async fn xfer_sending(socket: &mut TcpStream, buf2: &mut BufPair, data_len: usize, ) -> Result<()> { 
    let mut speed = TrafficSpeed::default();
    let mut traffic = Traffic::default();
    
    let data = vec![0_u8; data_len];

    loop {
        packet::encode_data(PacketType::Data, &data, &mut buf2.obuf)?;
        socket.write_buf(&mut buf2.obuf).await?;

        // socket.write_all_buf(&mut buf2.obuf).await?;
        // socket.flush().await?;

        traffic.inc(data_len as u64);

        if let Some(r) = speed.check_speed(Instant::now(), &traffic) {
            info!( "send speed: [{}]", r.human());
        }
    }
}

pub async fn xfer_recving(socket: &mut TcpStream, buf2: &mut BufPair,) -> Result<()> { 
    let mut speed = TrafficSpeed::default();
    let mut traffic = Traffic::default();

    loop {
        let header = read_packet(socket, &mut buf2.ibuf).await?;

        let len = header.offset+header.len;
        match header.ptype {
            x if x == PacketType::Data as u8 => {
                traffic.inc(len as u64);
                if let Some(r) = speed.check_speed(Instant::now(), &traffic) {
                    info!( "recv speed: [{}]", r.human());
                }
            },
            _ => bail!("xfering but got packet type {}", header.ptype),
        } 
        buf2.ibuf.advance(len);

    }

}

pub async fn read_specific_packet<R: AsyncReadExt+Unpin>(reader: &mut R, ptype: PacketType, buf: &mut BytesMut) -> Result<Header> {
    let r = read_packet(reader, buf).await?;

    if r.ptype != ptype as u8 {
        bail!("expect packet type [{:?}({})], but [{}]", ptype, ptype as u8, r.ptype)
    }
    Ok(r)
}

pub async fn read_packet<R: AsyncReadExt+Unpin>(reader: &mut R, buf: &mut BytesMut) -> Result<Header> {
    loop {
        let r = packet::is_completed(buf);
        match r {
            Some(r) => {
                return Ok(r)
            },
            None => {},
        }
        
        let n = reader.read_buf(buf).await?;
        if n == 0 {
            bail!("connection disconnected")
        }
    }
}
