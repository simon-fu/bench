
use std::future::Future;
use std::io::Result;
use std::net::SocketAddr;
use bytes::BytesMut;


pub trait AsyncReadBuf
{
    type Fut<'a>: Future< Output = Result<usize> >
    where
        Self: 'a;

    fn async_read_buf<'a>(&'a mut self, buf: &'a mut BytesMut) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait AsyncWriteBuf
{
    type Fut<'a>: Future< Output = Result<usize> >
    where
        Self: 'a;

    fn async_write_buf<'a>(&'a mut self, buf: &'a mut BytesMut) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait AsyncWriteAllBuf
{
    type Fut<'a>: Future< Output = Result<()> >
    where
        Self: 'a;

    fn async_write_all_buf<'a>(&'a mut self, buf: &'a mut BytesMut) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait AsyncFlush
{
    type Fut<'a>: Future< Output = Result<()> >
    where
        Self: 'a;

    fn async_flush<'a>(&'a mut self) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait AsyncReadable
{
    type Fut<'a>: Future< Output = Result<()> >
    where
        Self: 'a;

    fn async_readable<'a>(&'a mut self) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait AsyncShutdown
{
    type Fut<'a>: Future< Output = Result<()> >
    where
        Self: 'a;

    fn async_shutdown<'a>(&'a mut self) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}


pub trait AsyncBindAndConnect: Sized
{
    type Fut<'a>: Future< Output = Result<Self> >
    where
        Self: 'a;

    fn async_bind_and_connect<'a>(bind_addr: SocketAddr, target_addr: &'a str) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait GetLocalAddr {
    fn get_local_addr(&self) -> Result<SocketAddr>;
}


pub trait AsyncListen: Sized
{
    type Fut<'a>: Future< Output = Result<Self> >
    where
        Self: 'a;

    fn async_listen<'a>(bind_addr: SocketAddr) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait AsyncAccept
{
    type Stream: Sized + AsyncTcpStream;
    type Fut<'a>: Future< Output = Result<(Self::Stream, SocketAddr)> >
    where
        Self: 'a;

    fn async_accept<'a>(&'a self) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}


pub trait AsyncTcpStream : Unpin + AsyncWriteBuf + AsyncWriteAllBuf + AsyncFlush + AsyncReadable + AsyncShutdown + AsyncReadBuf + AsyncBindAndConnect + GetLocalAddr {
    
}

pub trait AsyncTcpListener : Unpin + AsyncListen + AsyncAccept + GetLocalAddr {
    
}

