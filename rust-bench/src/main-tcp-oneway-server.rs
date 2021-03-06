


mod xrs;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use xrs::speed::Speed;

use tokio::sync::mpsc;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{self, Instant};
use tracing::{error, info, debug};

use std::io::Cursor;
use std::sync::Arc;
use std::{time::Duration};
use clap::{Clap};

const CHECK_PRINT_INTERVAL: u64 = 1000;
const SPEED_REPORT_INTERVAL: u64 = 1000;
const SPEED_CAP_DURATION: i64 = 4000;

// #[derive(Debug)]
// enum Direction {
//     Send,
//     Recv,
// }

// impl FromStr for Direction {
//     type Err = &'static str;
//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         match s {
//             "send" => Ok(Direction::Send),
//             "recv" => Ok(Direction::Recv),
//             _ => Err("no match"),
//         }
//     }
// }


// refer https://github.com/clap-rs/clap/tree/master/clap_derive/examples
#[derive(Clap, Debug)]
#[clap(name="tcp one-way server", author, about, version)]
struct Config{
    #[clap(short='a', long, default_value = "127.0.0.1:7000", long_about="listen address.")]
    address: String, 

    #[clap(short='l', long, default_value = "512", long_about="buffer size")]
    length: usize,

    // #[clap(long, default_value = "recv", long_about="data direction")]
    // direction: Direction,

    #[clap(long, long_about="enable sending data")]
    enable_send: bool,

    #[clap(long, long_about="disable recving data")]
    disable_recv: bool,
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
    Xfer {ts: i64, ibytes:u32, obytes:u32},
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
            info!("Transfer: recv {} B ({} KB/s), send {} B ({} KB/s)", 
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
            SessionEvent::Xfer {ts, ibytes, obytes } => {
                self.bw.input.bytes += *ibytes as u64;
                self.bw.output.bytes += *obytes as u64;
                self.bw.input.speed.add(*ts, (*ibytes).into());
                self.bw.output.speed.add(*ts, (*obytes).into());
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


async fn session_entry(mut socket : TcpStream, cfg : Arc<Config>, tx: mpsc::Sender<SessionEvent>){
    let mut ibytes:u32 = 0;
    let mut obytes:u32 = 0;
    let mut in_buf = BytesMut::with_capacity(cfg.length);
    let mut out_buf = Cursor::new(vec![0; cfg.length]);
    let (mut rd, mut wr) = socket.split();
    let mut next_report_time = Instant::now() + Duration::from_millis(SPEED_REPORT_INTERVAL);

    loop {

        tokio::select! {
            r = rd.read_buf(&mut in_buf), if !cfg.disable_recv => {
                match r {
                    Ok(n) => {
                        if n == 0 {
                            break;
                        }

                        //trace!("read bytes {}", n);
                        ibytes += n as u32;
                        unsafe {
                            in_buf.set_len(0);
                        }
                    },
                    Err(e) => {
                        debug!("failed to read socket, error=[{}]", e);
                        break;
                    },
                }
            }

            r = wr.write_buf(&mut out_buf), if cfg.enable_send => {
                match r {
                    Ok(n) => {
                        if n == 0 {
                            break;
                        }

                        //trace!("written bytes {}", n);
                        obytes += n as u32;
                        out_buf.set_position(0);
                        
                    },
                    Err(e) => {
                        debug!("failed to write socket, error=[{}]", e);
                        break;
                    },
                }
            }

            _ = time::sleep_until(next_report_time) => {
                if ibytes > 0 || obytes > 0 {
                    let _=tx.send(SessionEvent::Xfer{ts:xrs::time::now_millis(), ibytes, obytes}).await;
                    ibytes = 0;
                    obytes = 0;
                }
                next_report_time = Instant::now() + Duration::from_millis(SPEED_REPORT_INTERVAL);
            }
        };
    }

    if ibytes > 0 || obytes > 0 {
        let _=tx.send(SessionEvent::Xfer{ts:xrs::time::now_millis(), ibytes, obytes}).await;
    }

    let _=tx.send(SessionEvent::Finished).await;
}



//#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    xrs::tracing_subscriber::init_simple_milli();

    let cfg0 = Config::parse();
    info!("cfg={:?}", cfg0);
    let cfg0 = Arc::new(cfg0);
    

    let listener = TcpListener::bind(&cfg0.address).await?;
    info!("tcp one-way server listening on {}", cfg0.address);

    let mut serv = Server::new();
    let (tx0, mut rx0) = mpsc::channel(10240);
    
    let mut next_print_time = Instant::now() + Duration::from_millis(CHECK_PRINT_INTERVAL);

    loop {

        tokio::select! {
            result = listener.accept() => {
                match result{
                    Ok((socket, _)) => {
                        let cfg = cfg0.clone();
                        let tx = tx0.clone();
                        serv.add_session();
                        tokio::spawn(async move {
                            session_entry(socket, cfg, tx).await;
                        });
                    },
                    Err(_) => {}, 
                } 
            }

            _ = time::sleep_until(next_print_time) => {
                serv.check_print_sessions();
                next_print_time = Instant::now() + Duration::from_millis(CHECK_PRINT_INTERVAL);
            }

            result = rx0.recv() => {
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

