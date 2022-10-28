



use std::{net::{TcpStream, Shutdown, TcpListener, SocketAddr}, io::{Write, Read}, time::{Instant, Duration}, collections::VecDeque};
use anyhow::{Result, Context, bail};
use bytes::{BytesMut, Buf};
use rust_bench::util::{traffic::{TrafficEstimator, Traffic, ToHuman}, now_millis};
use tracing::info;
use crate::{args::{ClientArgs, ServerArgs}, packet::{HandshakeRequest, self, PacketType, HandshakeResponse, Header, HandshakeResponseCode}};

#[derive(Debug)]
pub struct Slice2<'a> {
    slice1: &'a [u8],
    slice2: &'a [u8],
    offset: usize,
}

impl <'a> std::io::Read for Slice2<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remains = (self.slice1.len() + self.slice2.len()) - self.offset;
        
        let len = remains.min(buf.len());

        if len > 0 { 
            let n1 = if self.offset < self.slice1.len() {
                let n = (self.slice1.len() - self.offset).min(len);
                buf[..n].clone_from_slice(&self.slice1[self.offset..self.offset+n]);
                n
            } else {
                0
            };

            if n1 < len {
                let offset = self.offset - self.slice1.len();
                let end = offset + (len - n1);
                buf[n1..len].clone_from_slice(&self.slice2[offset..end]);
            }

            self.offset += len;
        }
        Ok(len)
    }
}

impl<'a> From<(&'a [u8], &'a [u8])> for Slice2<'a> {
    fn from(src: (&'a [u8], &'a [u8])) -> Self {
        Self {
            slice1: src.0,
            slice2: src.1,
            offset: 0,
        }
    }
}


#[derive(Default, Debug)]
pub struct VecBuf {
    buf: VecDeque<u8>,
    last: usize,
}

impl VecBuf {
    pub fn as_reader<'a>(&'a mut self) -> Slice2<'a> { 
        Slice2::from(self.as_slice())
    }

    pub fn remains(&self) -> usize { 
        self.last
    }

    pub fn as_slice(&self) -> (&[u8], &[u8]) {
        let (s1, s2) = self.buf.as_slices();

        if self.last <= s1.len() {
            (&s1[0..self.last], &s2[0..0])
        } else { 
            let end2 = self.last - s1.len();
            (&s1[..], &s2[0..end2])
        }
    }

    pub fn appendable(&mut self) -> &mut [u8] {
        let space = self.buf.len() - self.last;
        if space == 0 {
            if self.buf.capacity() > self.buf.len() {
                self.buf.resize(self.buf.capacity(), 0);    
            } else {
                self.buf.resize(self.buf.capacity()+ 1_000_000, 0);
            }
        }
        let (slice1, slice2) = self.buf.as_mut_slices();
        if self.last < slice1.len() {
            &mut slice1[self.last..]    
        } else {
            &mut slice2[self.last-slice1.len()..]
        }
    }

    pub fn extend(&mut self, n: usize) {
        self.last = (self.last + n).min(self.buf.len());
    }

    pub fn advance(&mut self, n: usize) {
        let len = self.remains().min(n);
        self.last -= len;
        self.buf.advance(len);
    }
}



#[derive(Default)]
pub struct BufPair {
    pub ibuf: VecBuf,
    pub obuf: BytesMut,
}

pub fn run_as_client(args: &ClientArgs) -> Result<()> 
{

    // let bind_addr: SocketAddr = format!("{}:{}", args.common.bind.as_deref().unwrap_or_else(||"0.0.0.0"), args.cport).parse()?; 

    info!("Connecting to [{}]...", args.target);
    let mut socket = TcpStream::connect(&args.target)?;
    
    info!("local [{}] connected to [{}]", socket.local_addr()?, &args.target);

    let mut buf2 = BufPair::default();

    let hreq = HandshakeRequest {
        ver: packet::VERSION,
        is_reverse: args.is_reverse,
        data_len: args.packet_len(),
        secs: args.secs,
        timestamp: now_millis(),
        pps: args.pps(),
    };

    packet::encode_json(PacketType::HandshakeRequest, &hreq, &mut buf2.obuf)?;
    socket.write_all(&mut buf2.obuf)?;
    buf2.obuf.clear();

    let header = read_specific_packet(&mut socket, PacketType::HandshakeResponse, &mut buf2.ibuf)?;

    // let data = &buf2.ibuf.as_ref()[header.offset..header.offset+header.len];
    // let rsp: HandshakeResponse = serde_json::from_slice(data).with_context(||"invalid handshake request")?;
    // buf2.ibuf.advance(header.offset+header.len);

    buf2.ibuf.advance(header.offset);
    let rsp: HandshakeResponse = serde_json::from_reader(buf2.ibuf.as_reader())?;
    buf2.ibuf.advance(header.len);
    
    if rsp.ver != packet::VERSION {
        bail!("expect version [{}] but [{}]", packet::VERSION, rsp.ver);
    }
    
    if !args.is_reverse {
        xfer_sending(&mut socket, &mut buf2, &hreq)?;
    } else {
        xfer_recving(&mut socket, &mut buf2)?;
    }

    socket.shutdown(Shutdown::Both)?;

    info!( "Done");

    Ok(())
    
}

pub fn run_as_server(args: &ServerArgs) -> Result<()> 
{ 

    let bind: SocketAddr = args.bind.parse()?;
    let listener = TcpListener::bind(bind)
    .with_context(||format!("fail to bind at [{}]", bind))?;

    let mut num = 0_u64;
    let mut cid = 0_u64;

    loop {
        num += 1;
        info!("-------------------------------------------------");
        info!("Server listening on {} (test #{})", listener.local_addr().with_context(||"no local addr")?, num);
        info!("-------------------------------------------------");

        let (mut socket, remote) = listener.accept().with_context(||"fail to accept")?;
        info!("Accepted connection from [{}]", remote);

        let mut buf2 = BufPair::default();
        cid += 1;

        let r = service_session(&mut socket, &mut buf2, cid);
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

fn service_session(socket: &mut TcpStream, buf2: &mut BufPair, cid: u64) -> Result<()> 
{
    let header = read_specific_packet(socket, PacketType::HandshakeRequest, &mut buf2.ibuf)?;

    buf2.ibuf.advance(header.offset);
    let hreq: HandshakeRequest = serde_json::from_reader(buf2.ibuf.as_reader()).with_context(||"parsing hreq")?;
    buf2.ibuf.advance(header.len);
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
        socket.write_all(&mut buf2.obuf)?;
        buf2.obuf.clear();
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
    socket.write_all(&mut buf2.obuf)?;
    buf2.obuf.clear();
    info!("hanshake done");

    if !hreq.is_reverse {
        xfer_recving(socket, buf2)
    } else {
        xfer_sending(socket, buf2, &hreq)
    }

}




pub fn xfer_sending(socket: &mut TcpStream, buf2: &mut BufPair, hreq: &HandshakeRequest) -> Result<()> 
{ 
    let mut estimator = TrafficEstimator::default();
    let mut traffic = Traffic::default();
    
    let data = vec![0_u8; hreq.data_len];
    let start = Instant::now();
    let duration = Duration::from_secs(hreq.secs as u64);
    while start.elapsed() < duration{
        packet::encode_payload(PacketType::Data, &data, &mut buf2.obuf)?;
        let n = socket.write(&mut buf2.obuf)?;
        buf2.obuf.advance(n);

        traffic.inc_traffic(hreq.data_len as i64);

        if let Some(r) = estimator.estimate(Instant::now(), &traffic) {
            info!( "send rate: [{}]", r.to_human());
        }
    }
    packet::encode_payload(PacketType::Data, &[], &mut buf2.obuf)?;
    socket.write_all(&mut buf2.obuf)?;
    buf2.obuf.clear();
    socket.flush()?;
    // socket.async_readable().await?;
    

    Ok(())
}

pub fn xfer_recving(socket: &mut TcpStream, buf2: &mut BufPair,) -> Result<()> 
{ 
    let mut estimator = TrafficEstimator::default();
    let mut traffic = Traffic::default();

    loop {
        let header = read_packet(socket, &mut buf2.ibuf)?;

        let len = header.offset+header.len;
        match header.ptype {
            x if x == PacketType::Data as u8 => {
                buf2.ibuf.advance(len);
                
                if header.len == 0 {
                    break;
                }

                traffic.inc_traffic(len as i64);
                if let Some(r) = estimator.estimate(Instant::now(), &traffic) {
                    info!( "recv rate: [{}]", r.to_human());
                }
            },
            _ => bail!("xfering but got packet type {}", header.ptype),
        } 
    }

    Ok(())

}

pub fn read_specific_packet(reader: &mut TcpStream, ptype: PacketType, buf: &mut VecBuf) -> Result<Header> 
{
    let r = read_packet(reader, buf)?;

    if r.ptype != ptype as u8 {
        bail!("expect packet type [{:?}({})], but [{}]", ptype, ptype as u8, r.ptype)
    }
    Ok(r)
}

pub fn read_packet(reader: &mut TcpStream, buf: &mut VecBuf) -> Result<Header> 
{
    loop {
        let r = packet::is_completed2(buf.as_slice());
        match r {
            Some(r) => {
                return Ok(r)
            },
            None => {},
        }
        

        let mut_buf = buf.appendable();

        let n = reader.read(mut_buf).with_context(||format!("read packet error (mut_buf.len {})", mut_buf.len()))?;
        if n == 0 {
            bail!("connection disconnected")
        }
        buf.extend(n);

    }
}

