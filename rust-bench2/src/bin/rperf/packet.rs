use bytes::{Buf, BufMut};
use serde_derive::{Serialize, Deserialize};
use anyhow::{Result, bail};

const HEADER_LEN: usize = 5;
pub const VERSION: u8 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeRequest {
    pub ver: u8,
    pub is_reverse: bool,
    pub data_len: usize,
    pub secs: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub ver: u8,
    pub code: u8,
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
    pub offset: usize, 
    pub len: usize,
}

pub fn is_completed(data: &[u8]) -> Option<Header> {
    if data.len() < HEADER_LEN {
        return None
    }
    let mut buf = data;
    let itype = buf.get_u8() as u8;
    let len = buf.get_u32() as usize;
    if buf.len() < len {
        return None
    }
    
    // let ptype: PacketType = itype.try_into().map_err(|_e|anyhow!("unknown packet type {}", itype))?;
    Some(Header {
        ptype: itype,
        offset: HEADER_LEN,
        len,
    })
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

pub fn encode_data<B>(ptype: PacketType, payload: &[u8], buf: &mut B) -> Result<()>
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
