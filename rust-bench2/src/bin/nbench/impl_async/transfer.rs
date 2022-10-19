use std::time::{Instant, Duration};
use anyhow::{Result, bail};
use bytes::{BytesMut, Buf, BufMut};
use rust_bench::util::{traffic::AtomicTraffic, async_rt::{async_tcp::{AsyncTcpStream2, AsyncReadBuf}}};
use crate::packet::{HandshakeRequest, self, PacketType, Header, BufPair};


pub async fn xfer_sending<S>(socket: &mut S, buf2: &mut BufPair, hreq: &HandshakeRequest, traffic: &AtomicTraffic) -> Result<()> 
where
    S: AsyncTcpStream2<BytesMut>,
{ 

    let data = vec![0_u8; hreq.data_len];
    let start = Instant::now();
    let duration = Duration::from_secs(hreq.secs);
    while start.elapsed() < duration{
        packet::encode_data(PacketType::Data, &data, &mut buf2.obuf)?;
        socket.async_write_buf(&mut buf2.obuf).await?;

        traffic.inc_traffic(hreq.data_len as i64);
    }
    packet::encode_data(PacketType::Data, &[], &mut buf2.obuf)?;
    socket.async_write_all_buf(&mut buf2.obuf).await?;
    socket.async_flush().await?;
    // socket.async_readable().await?;
    

    Ok(())
}

pub async fn xfer_recving<S>(socket: &mut S, buf2: &mut BufPair, traffic: &AtomicTraffic) -> Result<()> 
where
    S: AsyncTcpStream2<BytesMut>,
{ 

    loop {
        let header = read_packet(socket, &mut buf2.ibuf).await?;

        let len = header.offset+header.len;
        match header.ptype {
            x if x == PacketType::Data as u8 => {
                buf2.ibuf.advance(len);
                
                if header.len == 0 {
                    break;
                }

                traffic.inc_traffic(len as i64);
            },
            _ => bail!("xfering but got packet type {}", header.ptype),
        } 
    }

    Ok(())

}

pub async fn read_specific_packet<S, B>(reader: &mut S, ptype: PacketType, buf: &mut B) -> Result<Header> 
where
    S: Unpin + AsyncReadBuf<Buf = B>,
    B: BufMut + Buf,
{
    let r = read_packet(reader, buf).await?;

    if r.ptype != ptype as u8 {
        bail!("expect packet type [{:?}({})], but [{}]", ptype, ptype as u8, r.ptype)
    }
    Ok(r)
}

pub async fn read_packet<S, B>(reader: &mut S, buf: &mut B) -> Result<Header> 
where
    S: Unpin + AsyncReadBuf<Buf = B>,
    B: BufMut + Buf,
{
    loop {
        let r = packet::is_completed(buf.chunk());
        match r {
            Some(r) => {
                return Ok(r)
            },
            None => {},
        }
        
        let n = reader.async_read_buf(buf).await?;
        if n == 0 {
            bail!("connection disconnected")
        }
    }
}
