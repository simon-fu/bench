use bytes::{Buf, BufMut, BytesMut};
use rust_bench::util::now_millis;
use serde_derive::{Serialize, Deserialize};
use anyhow::{Result, bail};

const HEADER_LEN: usize = 5;
pub const VERSION: u8 = 1;

#[derive(Default)]
pub struct BufPair {
    pub ibuf: BytesMut,
    pub obuf: BytesMut,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeRequest {
    pub ver: u8,
    pub is_reverse: bool,
    pub data_len: usize,
    pub secs: u64,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub ver: u8,
    pub code: u8,
    pub timestamp: i64,
    pub cid: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HandshakeResponseCode {
    Success = 0, 
    Unknown = 1,
    VersionNotMatch = 2, 
    Busy = 3, 
}

impl From<i32> for HandshakeResponseCode {
    fn from(v: i32) -> Self {
        match v {
            x if x == Self::Success as i32 => Self::Success,
            x if x == Self::Busy as i32 => Self::Busy,
            x if x == Self::VersionNotMatch as i32 => Self::VersionNotMatch,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PacketType {
    HandshakeRequest = 1, 
    HandshakeResponse = 2, 
    Data = 3,
}

impl TryFrom<i32> for PacketType {
    type Error = anyhow::Error;

    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            x if x == Self::HandshakeRequest as i32 => Ok(Self::HandshakeRequest),
            x if x == Self::HandshakeResponse as i32 => Ok(Self::HandshakeResponse),
            x if x == Self::Data as i32 => Ok(Self::Data),
            _ => bail!("unknown packet type {}", v),
        }
    }
}

pub struct Header {
    pub ptype: u8, 
    pub offset: usize,  // payload offset
    pub len: usize,     // payload length
}

pub fn is_completed2((data1, data2): (&[u8], &[u8])) -> Option<Header> {
    
    let total_len = data1.len() + data2.len();

    if total_len < HEADER_LEN {
        return None
    }

    if data1.len() >= HEADER_LEN {
        return check_completed(data1, total_len)
    }

    let mut buf = [0_u8; HEADER_LEN];
    if data1.len() > 0 {
        buf[..data1.len()].clone_from_slice(&data1);
    }
    
    let remains = HEADER_LEN - data1.len();

    buf[data1.len()..].clone_from_slice(&data2[0..remains]);

    check_completed(&buf, total_len)

}

pub fn is_completed(data: &[u8]) -> Option<Header> {
    if data.len() < HEADER_LEN {
        return None
    }

    check_completed(data, data.len())

    // let mut buf = data;
    // let itype = buf.get_u8() as u8;
    // let len = buf.get_u32() as usize;
    // if buf.len() < len {
    //     return None
    // }

    // // let ptype: PacketType = itype.try_into().map_err(|_e|anyhow!("unknown packet type {}", itype))?;
    // Some(Header {
    //     ptype: itype,
    //     offset: HEADER_LEN,
    //     len,
    // })
}



fn check_completed(header: &[u8], total_len: usize) -> Option<Header> {
    let mut buf = header;
    let itype = buf.get_u8() as u8;
    let len = buf.get_u32() as usize;
    if total_len < (HEADER_LEN + len) {
        None
    } else {
        Some(Header {
            ptype: itype,
            offset: HEADER_LEN,
            len,
        })
    }
}


pub fn encode_json<T, B>(ptype: PacketType, value: &T, buf: &mut B) -> Result<()>
where
    T: ?Sized + serde::Serialize,
    B: BufMut,
{
    let payload = serde_json::to_vec(&value)?;
    buf.put_u8(ptype as u8);
    buf.put_u32(payload.len() as u32);
    buf.put_slice(&payload);

    Ok(())
}

pub fn encode_payload<B>(ptype: PacketType, payload: &[u8], buf: &mut B) -> Result<()>
where
    B: BufMut,
{
    buf.put_u8(ptype as u8);
    buf.put_u32(payload.len() as u32);
    if payload.len() > 0 {
        buf.put_slice(&payload);
    }
    Ok(())
}

fn encode_ts_dummy<B>(ptype: PacketType, ts: i64, total_len: usize, buf: &mut B) -> Result<()>
where
    B: BufMut,
{
    const MIN_LEN: usize = 1+4+8;
    
    let len = if total_len >= MIN_LEN {
        total_len - MIN_LEN
    } else {
        0
    };

    buf.put_u8(ptype as u8);
    buf.put_u32((8+len) as u32);
    buf.put_i64(ts);
    // unsafe { buf.advance_mut(len) };
    buf.put_bytes(0, len);
    Ok(())
}

pub fn encode_ts_data<B>(total_len: usize, buf: &mut B) -> Result<()> 
where
    B: BufMut,
{
    encode_ts_dummy(PacketType::Data, now_millis(), total_len, buf)
}

pub fn encode_ts_data_last<B>(buf: &mut B) -> Result<()> 
where
    B: BufMut,
{
    encode_ts_dummy(PacketType::Data, now_millis(), 8, buf)
}

pub fn decode_ts_dummy<B>(len: usize, buf: &mut B) -> Result<i64>
where
    B: Buf,
{
    if len < 8 || buf.remaining() < len {
        bail!("NOT enough remaining for ts, {:?}", (len, buf.remaining()))
    }
    let ts = buf.get_i64();
    buf.advance(len-8);
    Ok(ts)
}


// struct BufWriter<'a, B>(&'a mut B);
// impl<'a, B> std::io::Write for BufWriter<'a, B> 
// where
//     B: BufMut,
// {
//     fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
//         self.0.put_slice(buf);
//         Ok(buf.len())
//     }

//     fn flush(&mut self) -> std::io::Result<()> {
//         Ok(())
//     }
// }
