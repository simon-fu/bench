


mod xrs;
use bytes::Buf;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use xrs::speed::Speed;

use tokio::sync::mpsc;
use tokio::sync::broadcast;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{self, Instant};
use tracing::{error, info, debug};

use std::collections::HashMap;
use std::sync::Arc;
use std::{time::Duration};
use clap::{Clap};
use bytes::Bytes;

const CHECK_PRINT_INTERVAL: u64 = 1000;
const SPEED_REPORT_INTERVAL: u64 = 1000;
const SPEED_CAP_DURATION: i64 = 4000;

#[derive(Debug)]
enum Mode {
    Hub,
    Broadcast,
}

impl std::str::FromStr for Mode {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hub" => Ok(Mode::Hub),
            "broadcast" => Ok(Mode::Broadcast),
            _ => Err("no match"),
        }
    }
}

// refer https://github.com/clap-rs/clap/tree/master/clap_derive/examples
#[derive(Clap, Debug)]
#[clap(name="tcp fanout server", author, about, version)]
struct Config{
    #[clap(short='a', long, default_value = "127.0.0.1:7000", long_about="listen address.")]
    address: String, 

    #[clap(short='l', long, default_value = "512", long_about="buffer size")]
    length: usize,

    #[clap(short='m', long, default_value = "hub", long_about="mode [hub, broadcast].")]
    mode: Mode
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
    Finished {pid:u32} ,
}

#[derive(Clone)]
enum BcastEvent {
    Data { buf : Bytes},
}


struct Server{
    conns : ConnsStati,
    bw : BWStati,
    last_print_time : Instant,
    hub : Arc<Hub>
}

impl Server{
    fn new()->Self{
        Server{
            conns: (ConnsStati::new()),
            bw: (BWStati::new()),
            last_print_time : Instant::now(),
            hub : Arc::new(Hub::default())
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

    async fn process_ev(self: &mut Self, ev : &SessionEvent) {
        //trace!("process_ev {:?}", ev);
        match ev {
            SessionEvent::Xfer {ts, ibytes, obytes } => {
                self.bw.input.bytes += *ibytes as u64;
                self.bw.output.bytes += *obytes as u64;
                self.bw.input.speed.add(*ts, (*ibytes).into());
                self.bw.output.speed.add(*ts, (*obytes).into());
                self.bw.updated = true;
            },

            SessionEvent::Finished { pid } => {
                self.conns.count -= 1;
                self.conns.updated = true;
                self.hub.remove(*pid).await;
            }
        };
    }
    
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct PTask{
    pid : u32,
    tx: mpsc::Sender<BcastEvent>,
}

#[derive(Default,Debug)]
struct Hub{
    tasks : RwLock<HashMap<u32, PTask>>,
}

impl Hub {
    async fn broadcast(&self, pid:u32, ev : BcastEvent){
        let tasks = self.tasks.read().await;
        for (ppid, task) in &(*tasks) {
            if *ppid != pid {
                let r  = task.tx.send(ev.clone()).await;
                match r {
                    Ok(_) => {},
                    Err(e) => {error!("broadcast to task fail with [{}]", e);},
                }
            }

        }
    }

    async fn add(&self, task : PTask){
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.pid, task);
    }

    async fn remove(&self, pid : u32){
        let mut tasks = self.tasks.write().await;
        tasks.remove(&pid);
    }
}


async fn session_entry_broadcast(pid: u32, mut socket : TcpStream, cfg : Arc<Config>, tx: mpsc::Sender<SessionEvent>, tx_bc : broadcast::Sender<BcastEvent>, mut rx_bc : broadcast::Receiver<BcastEvent>){
    let mut ibytes:u32 = 0;
    let mut obytes:u32 = 0;
    let mut in_buf = BytesMut::with_capacity(cfg.length);
    let (mut rd, mut wr) = socket.split();
    let mut next_report_time = Instant::now() + Duration::from_millis(SPEED_REPORT_INTERVAL);
    let mut out_buf = Bytes::new();
    
    loop {

        tokio::select! {
            r = rd.read_buf(&mut in_buf) => {
                match r {
                    Ok(n) => {
                        if n == 0 {
                            break;
                        }

                        //trace!("read bytes {}", n);
                        ibytes += n as u32;

                        if in_buf.len() == cfg.length {
                            let buf = in_buf.freeze();
                            in_buf = BytesMut::with_capacity(cfg.length);
                            let _ = tx_bc.send(BcastEvent::Data { buf});
                        }
                    },
                    Err(e) => {
                        debug!("failed to read socket, error=[{}]", e);
                        break;
                    },
                }
            }

            r = wr.write_buf(&mut out_buf), if out_buf.remaining() > 0 => {
                match r {
                    Ok(n) => {
                        //trace!("written bytes {}", n);
                        obytes += n as u32;
                    },
                    Err(e) => {
                        debug!("failed to write socket, error=[{}]", e);
                        break;
                    },
                }
            }


            r = rx_bc.recv(), if out_buf.remaining() == 0 => {
                match r {
                    Ok(ev) => {
                        match ev{
                            BcastEvent::Data { buf } => {
                                //trace!("from broadcast bytes {}", buf.remaining());
                                out_buf = buf;
                            },
                        }
                    },
                    Err(e) => {
                        debug!("failed to recv broadcast, error=[{}]", e);
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

    let _=tx.send(SessionEvent::Finished{pid}).await;
}

fn spawn_tasks_broadcast(pid:u32, socket : TcpStream, cfg0 : &Arc<Config>, tx0: &mpsc::Sender<SessionEvent>, tx_bc0 : &broadcast::Sender<BcastEvent>,){
    let cfg = cfg0.clone();
    let tx = tx0.clone();
    let tx_bc = tx_bc0.clone();
    let rx_bc = tx_bc0.subscribe();
    tokio::spawn(async move {
        session_entry_broadcast(pid, socket, cfg, tx, tx_bc, rx_bc).await;
    });
}



async fn session_entry_hub(pid:u32, mut socket : TcpStream, cfg : Arc<Config>, tx: mpsc::Sender<SessionEvent>, mut rx_data : mpsc::Receiver<BcastEvent>, hub : Arc<Hub>){
    let mut ibytes:u32 = 0;
    let mut obytes:u32 = 0;
    let mut in_buf = BytesMut::with_capacity(cfg.length);
    let (mut rd, mut wr) = socket.split();
    let mut next_report_time = Instant::now() + Duration::from_millis(SPEED_REPORT_INTERVAL);
    let mut out_buf = Bytes::new();
    
    loop {
        tokio::select! {
            r = rd.read_buf(&mut in_buf) => {
                match r {
                    Ok(n) => {
                        if n == 0 {
                            break;
                        }

                        //trace!("read bytes {}", n);
                        ibytes += n as u32;

                        if in_buf.len() == cfg.length {
                            let buf = in_buf.freeze();
                            in_buf = BytesMut::with_capacity(cfg.length);
                            hub.broadcast(pid, BcastEvent::Data { buf}).await;
                        }
                    },
                    Err(e) => {
                        debug!("failed to read socket, error=[{}]", e);
                        break;
                    },
                }
            }

            r = wr.write_buf(&mut out_buf), if out_buf.remaining() > 0 => {
                match r {
                    Ok(n) => {
                        //trace!("written bytes {}", n);
                        obytes += n as u32;
                    },
                    Err(e) => {
                        debug!("failed to write socket, error=[{}]", e);
                        break;
                    },
                }
            }


            r = rx_data.recv(), if out_buf.remaining() == 0 => {
                match r {
                    Some(ev) => {
                        match ev{
                            BcastEvent::Data { buf } => {
                                //trace!("from broadcast bytes {}", buf.remaining());
                                out_buf = buf;
                            },
                        }
                    },
                    None => {
                        error!("recv broadcast data but got None");
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

    let _=tx.send(SessionEvent::Finished{pid}).await;
}

fn spawn_tasks_hub(pid:u32, socket : TcpStream, cfg0 : &Arc<Config>, tx0: &mpsc::Sender<SessionEvent>, rx_data : mpsc::Receiver<BcastEvent>, hub : Arc<Hub>){
    let cfg = cfg0.clone();
    let tx = tx0.clone();
    tokio::spawn(async move {
        session_entry_hub(pid, socket, cfg, tx, rx_data, hub).await;
    });
}


//#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    xrs::tracing_subscriber::init_simple_milli();

    let cfg0 = Config::parse();
    info!("cfg={:?}", cfg0);
    let cfg0 = Arc::new(cfg0);
    

    let listener = TcpListener::bind(&cfg0.address).await?;
    info!("tcp fanout server listening on {}", cfg0.address);

    let mut serv = Server::new();
    let (tx0, mut rx0) = mpsc::channel(10240);
    let (tx_bc0, _) = broadcast::channel(10240);

    let mut pid:u32 = 0 ;
    let mut next_print_time = Instant::now() + Duration::from_millis(CHECK_PRINT_INTERVAL);


    loop {

        tokio::select! {
            result = listener.accept() => {
                match result{
                    Ok((socket, _)) => {
                        pid += 1;
                        serv.add_session();
                        if matches!(cfg0.mode, Mode::Hub) {
                            let (tx, rx) = mpsc::channel(1024);
                            serv.hub.add(PTask{pid, tx}).await;
                            spawn_tasks_hub(pid, socket, &cfg0, &tx0, rx, serv.hub.clone());
                        } else {
                            spawn_tasks_broadcast(pid, socket, &cfg0, &tx0, &tx_bc0);
                        }
   
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
                        serv.process_ev(&ev).await;
                    },
                    None => {
                        error!("expect session event but got None ")
                    },
                }
            }
        };
    }
}

