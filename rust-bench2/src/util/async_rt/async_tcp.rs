
use std::{future::Future, time::Duration};
use std::io::Result;
use std::net::SocketAddr;
use bytes::{BufMut, Buf};


pub trait AsyncReadBuf
{
    type Fut<'a>: Future< Output = Result<usize> > + Send
    where
        Self: 'a;
    
    type Buf: BufMut;

    fn async_read_buf<'a>(&'a mut self, buf: &'a mut Self::Buf) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait AsyncWriteBuf
{
    type Fut<'a>: Future< Output = Result<usize> > + Send
    where
        Self: 'a;

    type Buf: Buf;

    fn async_write_buf<'a>(&'a mut self, buf: &'a mut Self::Buf) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait AsyncWriteAllBuf
{
    type Fut<'a>: Future< Output = Result<()> > + Send
    where
        Self: 'a;

    type Buf: Buf;

    fn async_write_all_buf<'a>(&'a mut self, buf: &'a mut Self::Buf) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait AsyncFlush
{
    type Fut<'a>: Future< Output = Result<()> > + Send
    where
        Self: 'a;

    fn async_flush<'a>(&'a mut self) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait AsyncReadable
{
    type Fut<'a>: Future< Output = Result<()> > + Send
    where
        Self: 'a;

    fn async_readable<'a>(&'a mut self) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait AsyncShutdown
{
    type Fut<'a>: Future< Output = Result<()> > + Send
    where
        Self: 'a;

    fn async_shutdown<'a>(&'a mut self) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}


pub trait AsyncBindAndConnect: Sized
{
    type Fut<'a>: Future< Output = Result<Self> > + Send
    where
        Self: 'a;

    fn async_bind_and_connect<'a>(bind_addr: Option<SocketAddr>, target_addr: &'a str) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait GetLocalAddr {
    fn get_local_addr(&self) -> Result<SocketAddr>;
}


// pub trait Spawn
// {
//     fn spawn<Fut1, Fut2>(future: Fut1) ->Fut2
//     where
//         Fut1: Future + Send + 'static,
//         Fut1::Output: Send + 'static,
//         Fut2: Future< Output = Result<Fut1::Output> >,
//     ;
// }


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

pub trait AsyncAccept2<B1: BufMut+Buf, B2: Buf = B1> 
{
    type Stream: Sized + AsyncTcpStream2<B1, B2>;
    type Fut<'a>: Future< Output = Result<(Self::Stream, SocketAddr)> >
    where
        Self: 'a;

    fn async_accept<'a>(&'a self) -> Self::Fut<'a>
    where
        Self: Sized + Unpin;
}

pub trait AsyncTcpStream
: Unpin 
+ Send + 'static 
+ AsyncReadBuf  
+ AsyncWriteBuf
+ AsyncWriteAllBuf
+ AsyncFlush 
+ AsyncReadable
+ AsyncShutdown 
+ AsyncBindAndConnect 
+ GetLocalAddr 
{

}

pub trait AsyncTcpStream2<B1: BufMut+Buf, B2: Buf = B1> 
: Unpin 
+ Send + 'static 
+ AsyncReadBuf<Buf = B1>  
+ AsyncWriteBuf<Buf = B2> 
+ AsyncWriteAllBuf<Buf = B2> 
+ AsyncFlush 
+ AsyncReadable
+ AsyncShutdown 
+ AsyncBindAndConnect 
+ GetLocalAddr 
{

}


pub trait AsyncTcpListener
: Unpin 
+ AsyncListen 
+ AsyncAccept 
+ GetLocalAddr 
{
    
}

pub trait AsyncTcpListener2<B1: BufMut+Buf, B2: Buf = B1>
: Unpin 
+ AsyncListen 
+ AsyncAccept2<B1, B2> 
+ GetLocalAddr 
{
    
}

pub trait AsyncSleep
{
    type Fut: Future< Output = () > + Send;

    fn async_sleep(duration: Duration) -> Self::Fut;
}

pub trait VRuntime: Unpin + AsyncSleep {
    type TaskHandle<O>: Future;

    fn spawn<Fut>(future: Fut) -> Self::TaskHandle<Fut::Output>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
    ;

}


