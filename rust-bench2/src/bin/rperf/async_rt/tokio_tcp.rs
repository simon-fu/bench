
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, lookup_host, TcpSocket, ToSocketAddrs, TcpListener};
use std::future::Future;
use std::io::Result;
use std::net::SocketAddr;
use super::async_tcp::{AsyncReadBuf, AsyncWriteBuf, AsyncWriteAllBuf, AsyncFlush, AsyncReadable, AsyncBindAndConnect, GetLocalAddr, AsyncShutdown, AsyncTcpStream, AsyncListen, AsyncAccept, AsyncTcpListener};


impl AsyncReadBuf for TcpStream {
    type Fut<'a>
    = impl Future< Output = Result<usize> > where Self: 'a;

    #[inline]
    fn async_read_buf<'a>(&'a mut self, buf: &'a mut BytesMut) -> Self::Fut<'_>
    where
        Self: Sized + Unpin
    {
        self.read_buf(buf)
    }
}

impl AsyncWriteBuf for TcpStream {
    type Fut<'a>
    = impl Future< Output = Result<usize> > where Self: 'a;

    #[inline]
    fn async_write_buf<'a>(&'a mut self, buf: &'a mut BytesMut) -> Self::Fut<'_>
    where
        Self: Sized + Unpin
    {
        self.write_buf(buf)
    }
}

impl AsyncWriteAllBuf for TcpStream {
    type Fut<'a>
    = impl Future< Output = Result<()> > where Self: 'a;

    #[inline]
    fn async_write_all_buf<'a>(&'a mut self, buf: &'a mut BytesMut) -> Self::Fut<'_>
    where
        Self: Sized + Unpin
    {
        self.write_all_buf(buf)
    }
}

impl AsyncFlush for TcpStream {
    type Fut<'a>
    = impl Future< Output = Result<()> > where Self: 'a;

    #[inline]
    fn async_flush<'a>(&'a mut self) -> Self::Fut<'_>
    where
        Self: Sized + Unpin
    {
        self.flush()
    }
}

impl AsyncReadable for TcpStream {
    type Fut<'a>
    = impl Future< Output = Result<()> > where Self: 'a;

    #[inline]
    fn async_readable<'a>(&'a mut self) -> Self::Fut<'_>
    where
        Self: Sized + Unpin
    {
        self.readable()
    }
}

impl AsyncShutdown for TcpStream {
    type Fut<'a>
    = impl Future< Output = Result<()> > where Self: 'a;

    #[inline]
    fn async_shutdown<'a>(&'a mut self) -> Self::Fut<'_>
    where
        Self: Sized + Unpin
    {
        self.shutdown()
    }
}

impl AsyncBindAndConnect for TcpStream {
    type Fut<'a>
    = impl Future< Output = Result<Self> > where Self: 'a;

    #[inline]
    fn async_bind_and_connect<'a>(bind_addr: SocketAddr, target_addr: &str) -> Self::Fut<'_>
    where
        Self: Sized + Unpin,
    {
        tcp_bind_and_connect(bind_addr, target_addr)
    }
}

async fn tcp_bind_and_connect<A>(bind_addr: SocketAddr, target_addr: A) -> std::io::Result<TcpStream> 
where
    A: ToSocketAddrs
{
    let mut connect_result = Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Not found target addr"));
    for addr in lookup_host(&target_addr).await? {
        let socket = TcpSocket::new_v4()?;
        socket.set_reuseaddr(true)?;
        // socket.set_reuseport(true)?;
        socket.bind(bind_addr)?;
        connect_result = socket.connect(addr).await;
        if connect_result.is_ok() {
            break;
        }
    }
    let stream = connect_result?;
    // stream.set_nodelay(false)?;
    Ok(stream)
}

impl GetLocalAddr for TcpStream {
    fn get_local_addr(&self) -> Result<SocketAddr> {
        self.local_addr()
    }
}

impl AsyncTcpStream for TcpStream {}



impl AsyncListen for TcpListener {
    type Fut<'a>
    = impl Future< Output = Result<Self> > where Self: 'a;

    #[inline]
    fn async_listen<'a>(bind_addr: SocketAddr) -> Self::Fut<'a>
    where
        Self: Sized + Unpin,
    {
        Self::bind(bind_addr)
    }
}

impl AsyncAccept for TcpListener {
    type Stream = TcpStream;
    type Fut<'a>
    = impl Future< Output = Result<(Self::Stream, SocketAddr)> > where Self: 'a;

    #[inline]
    fn async_accept<'a>(&'a self) -> Self::Fut<'_>
    where
        Self: Sized + Unpin,
    {
        self.accept()
    }
}

impl GetLocalAddr for TcpListener {
    fn get_local_addr(&self) -> Result<SocketAddr> {
        self.local_addr()
    }
}


impl AsyncTcpListener for TcpListener {}

