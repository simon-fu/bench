// https://github.com/tokio-rs/tokio/blob/master/examples/echo.rs

#![warn(rust_2018_idioms)]

mod xrs;
use xrs::speed::Speed;

use tokio::sync::mpsc;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{self, Instant};
use tokio::{io::{self, AsyncWriteExt, Interest}};
use tracing::{error, info, debug};
use std::{error::Error, time::Duration};
use clap::{Clap};

const CHECK_PRINT_INTERVAL: u64 = 1000;
const SPEED_REPORT_INTERVAL: u64 = 1000;
const SPEED_CAP_DURATION: i64 = 4000;

// refer https://github.com/clap-rs/clap/tree/master/clap_derive/examples
#[derive(Clap, Debug)]
#[clap(name="tcp echo server", author, about, version)]
struct Config{
    #[clap(short='a', long, default_value = "127.0.0.1:7000", long_about="listen address.")]
    address: String, 

    #[clap(short='l', long, default_value = "512", long_about="buffer size")]
    length: usize,
}

struct Bandwidth{
    bytes : u64,
    speed : Speed,
}

impl Bandwidth{
    fn new()->Self{
        Bandwidth{
            bytes : 0,
            speed : Speed::default(),
        }
    }

    fn clear(&mut self) {
        self.bytes = 0;
        self.speed.clear();
    }
}

struct ConnsStati{
    count : u32,
    speed : Speed,
    updated : bool,
}

impl ConnsStati{
    fn new()->Self{
        ConnsStati{
            count: (0),
            speed: (Speed::default()),
            updated: (false),
        }
    }

    fn clear(&mut self) {
        self.count = 0;
        self.speed.clear();
        self.updated = false;
    }
}

struct BWStati{
    input:Bandwidth,
    output:Bandwidth,
    updated:bool
}

impl BWStati{
    fn new()->Self{
        BWStati{
            input: Bandwidth::new(),
            output: Bandwidth::new(),
            updated: false,
        }
    }

    fn clear(&mut self) {
        self.input.clear();
        self.output.clear();
        self.updated = false;
    }
}

enum SessionEvent {
    Xfer { ibytes:u32, obytes:u32},
    Finished ,
}


struct Server{
    conns : ConnsStati,
    bw : BWStati,
    last_print_time : Instant,
}

impl Server{
    fn new()->Self{
        Server{
            conns: (ConnsStati::new()),
            bw: (BWStati::new()),
            last_print_time : Instant::now(),
        }
    }

    fn clear(self: &mut Self){
        self.conns.clear();
        self.bw.clear();
    }

    fn add_session(self: &mut Self){
        self.conns.speed.add(xrs::time::now_millis(), 1);
        self.conns.count += 1;
        self.conns.updated = true;
    }

    fn check_print_sessions(self: &mut Self){
        if self.last_print_time.elapsed().as_millis() < CHECK_PRINT_INTERVAL.into() {
            return;
        }
        
        let mut is_print = false;

        if self.bw.updated {
            self.bw.updated = false;
            is_print = true;
            info!("Transfer:  recv {} B ({} KB/s),  send {} B ({} KB/s)", 
                self.bw.input.bytes, self.bw.input.speed.cap_average(SPEED_CAP_DURATION, xrs::time::now_millis())/1000, 
                self.bw.output.bytes, self.bw.output.speed.cap_average(SPEED_CAP_DURATION, xrs::time::now_millis())/1000);
        }

        if self.conns.updated {
            self.conns.updated = false;
            is_print = true;

            info!("Connections: total {}, average {} c/s", self.conns.count, self.conns.speed.cap_average(SPEED_CAP_DURATION, xrs::time::now_millis()));
    
            if self.conns.count == 0 {
                info!("input bytes {}, output bytes {}", self.bw.input.bytes, self.bw.output.bytes);
                info!("no session exist, clear up");
                info!("");
                self.clear();
            }
        } 

        if is_print{
            self.last_print_time = Instant::now();
        }

    }

    fn process_ev(self: &mut Self, ev : &SessionEvent) {
        //trace!("process_ev {:?}", ev);
        match ev {
            SessionEvent::Xfer { ibytes, obytes } => {
                self.bw.input.bytes += *ibytes as u64;
                self.bw.output.bytes += *obytes as u64;
                self.bw.input.speed.add(xrs::time::now_millis(), (*ibytes).into());
                self.bw.output.speed.add(xrs::time::now_millis(), (*ibytes).into());
                self.bw.updated = true;
            },

            SessionEvent::Finished {  } => {
                self.conns.count -= 1;
                self.conns.updated = true;
            }
        };
    }
    
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

async fn session_entry(mut socket : TcpStream, buf_size : usize, tx0: mpsc::Sender<SessionEvent>){
    let mut ibytes:u32 = 0;
    let mut obytes:u32 = 0;
    let mut last_report_time = Instant::now();
    loop {
        let ready = socket.ready(Interest::READABLE).await.expect("fail to check socket ready state");

        if ready.is_readable() {
            let mut buf = vec![0; buf_size];

            match socket.try_read(&mut buf) {
                Ok(n) => {
                    
                    if n == 0 {
                        // gracefully closed
                        break;
                    }
                    ibytes += n as u32;
    
                    let result = socket.write_all(&buf[0..n]).await;
                    match result {
                        Ok(_) => {obytes += n as u32;},
                        Err(e) => {
                            debug!("failed to write data to socket, error=[{}]", e);
                            break;
                        },
                    }
                }

                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    continue;
                }

                Err(e) => {
                    debug!("failed to read data from socket, error=[{}]", e);
                    break;
                }
            }

            if last_report_time.elapsed().as_millis() >= SPEED_REPORT_INTERVAL.into() {
                if ibytes > 0 || obytes > 0 {
                    let _=tx0.send(SessionEvent::Xfer{ ibytes, obytes}).await;
                    ibytes = 0;
                    obytes = 0;
                }
                last_report_time = Instant::now();
            }


        }
    }

    if ibytes > 0 || obytes > 0 {
        let _=tx0.send(SessionEvent::Xfer{ ibytes, obytes}).await;
    }

    let _=tx0.send(SessionEvent::Finished).await;
}



//#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    xrs::tracing_subscriber::init_simple_milli();

    let cfg = Config::parse();
    info!("cfg={:?}", cfg);
    let buf_size = cfg.length;
    

    let listener = TcpListener::bind(&cfg.address).await?;
    info!("tcp echo server listening on {}", cfg.address);

    let mut serv = Server::new();
    let (tx, mut rx) = mpsc::channel(1024);
    let mut next_print_time = Instant::now() + Duration::from_millis(CHECK_PRINT_INTERVAL);

    loop {

        tokio::select! {
            result = listener.accept() => {
                match result{
                    Ok((socket, _)) => {
                        let tx0 = tx.clone();
                        serv.add_session();
                        tokio::spawn(async move {
                            session_entry(socket, buf_size, tx0).await;
                        });
                    },
                    Err(_) => {}, 
                } 
            }

            _ = time::sleep_until(next_print_time) => {
                serv.check_print_sessions();
                next_print_time = Instant::now() + Duration::from_millis(CHECK_PRINT_INTERVAL);
            }

            result = rx.recv() => {
                match result {
                    Some(ev) => {
                        serv.process_ev(&ev);
                    },
                    None => {
                        error!("expect session event but got None ")
                    },
                }
            }
        };
    }
}

