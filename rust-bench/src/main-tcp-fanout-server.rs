

// Test cases:
// - packets number
//      cargo run --release --bin tcp-fanout-server -- -l 1000
//      cargo run --release --bin tcp-bench -- -a 127.0.0.1:7000 -c 1000 -s 300 -t 99999 -l 1000
//      cargo run --release --bin tcp-bench -- -c 2 -t 999999 -l 1000 -p 500


mod xrs;
use xrs::speed::Speed;
use bytes::Buf;
use bytes::BufMut;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::select;
use tokio::sync::RwLock;
use tokio::sync::watch;
use tokio::sync::mpsc;
use tokio::sync::broadcast;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{self, Instant};
use tracing::{error, info, debug};

use std::collections::HashMap;
use std::process::exit;
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
    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
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

    #[clap(short='l', long, default_value = "512", long_about="packet length")]
    length: usize,

    #[clap(short='b', long="buffer", default_value = "1024", long_about="buffer size")]
    buf_size: usize,

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
    Xfer {ts: i64, ibytes:u64, obytes:u64},
    Finished {pid:u32} ,
}

#[derive(Clone, Debug)]
enum BcastEvent {
    Data { packet : Bytes},
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

    fn process_ev(self: &mut Self, ev : &SessionEvent) {
        match ev {
            SessionEvent::Xfer {ts, ibytes, obytes } => {
                self.bw.input.bytes += *ibytes as u64;
                self.bw.output.bytes += *obytes as u64;
                self.bw.input.speed.add(*ts, (*ibytes) as i64);
                self.bw.output.speed.add(*ts, (*obytes)as i64);
                self.bw.updated = true;
            },

            SessionEvent::Finished { pid:_pid } => {
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
                    Err(_) => {
                        //debug!("broadcast to task error [{}], pid {}", e, *ppid);
                    },
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
    let mut ibytes:u64 = 0;
    let mut obytes:u64 = 0;
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
                        ibytes += n as u64;

                        if in_buf.len() == cfg.length {
                            let packet = in_buf.freeze();
                            in_buf = BytesMut::with_capacity(cfg.length);
                            let _ = tx_bc.send(BcastEvent::Data { packet });
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
                        obytes += n as u64;
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
                            BcastEvent::Data { packet } => {
                                //trace!("from broadcast bytes {}", buf.remaining());
                                out_buf = packet;
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





struct ULinkHalf<'a>{
    cfg : &'a Arc<Config>,
    pid : u32,
    rd : tokio::net::tcp::ReadHalf<'a>,
    hub : &'a Arc<Hub>,
    ibytes : u64,
    obytes : u64,
}

impl<'a> ULinkHalf<'a> {
    fn news(
        cfg : &'a Arc<Config>,
        pid : u32,
        rd : tokio::net::tcp::ReadHalf<'a>,
        hub : &'a Arc<Hub>,
    ) -> Self {

        ULinkHalf{
            cfg,
            pid,
            rd,
            hub,
            ibytes: 0,
            obytes: 0,
        }
    }
    
    async fn run(&mut self, mut watch_rx0 : watch::Receiver<i32>) {
        let mut in_buf = BytesMut::with_capacity(self.cfg.buf_size);
        loop {
            if self.run_once(&mut watch_rx0, &mut in_buf).await {
                break;
            }
        }

        while in_buf.len() >= self.cfg.length {
            self.hub.broadcast(self.pid, BcastEvent::Data {packet:in_buf.split_to(self.cfg.length).freeze()}).await;
        }

        // info!("pid {}: ulink run done", self.pid);
    }

    async fn run_once(&mut self, watch_rx0 : &mut watch::Receiver<i32>, in_buf : &mut BytesMut) -> bool {
        
        let mut done = false;
        let (mut has_packet, action) = if in_buf.len() >= self.cfg.length {
            (true, self.hub.broadcast(self.pid, BcastEvent::Data {packet:in_buf.split_to(self.cfg.length).freeze()}))
        } else {
            (false, self.hub.broadcast(self.pid, BcastEvent::Data {packet:Bytes::new()}))
        };

        tokio::pin!(action);

        loop {
            select! {
                r = self.rd.read_buf(in_buf), if in_buf.len() < self.cfg.buf_size => {
                    match r {
                        Ok(n) => {
                            if n== 0 {
                                done = true;
                                break;
                            }
                            self.ibytes += n as u64;
                            if !has_packet && in_buf.len() >= self.cfg.length {
                                break;
                            } 
                        },
                        Err(e) => {
                            error!("read socket error [{}]", e);
                            break;
                        },
                    }
                }

                _ = &mut action, if has_packet => {
                    self.obytes = self.cfg.length as u64;
                    has_packet = false;
                    break;
                }

                _ = watch_rx0.changed() => {
                    if *watch_rx0.borrow() > 0{
                        done = true;
                        break;
                    } 
                }
            }
        }

        if has_packet {
            action.await;
            self.obytes = self.cfg.length as u64;
        }

        return done
    }
}


struct DLinkHalf<'a>{
    cfg : &'a Arc<Config>,
    pid : u32,
    wr : tokio::net::tcp::WriteHalf<'a>,
    rx_data : &'a mut mpsc::Receiver<BcastEvent>,
    ibytes : u64,
    obytes : u64,
}

impl<'a> DLinkHalf<'a> {
    fn news(
        cfg : &'a Arc<Config>,
        pid : u32,
        wr : tokio::net::tcp::WriteHalf<'a>,
        rx_data : &'a mut mpsc::Receiver<BcastEvent>,
    ) -> Self {

        DLinkHalf{
            cfg,
            pid,
            wr,
            rx_data,
            ibytes: 0,
            obytes: 0,
        }
    }

    async fn run(&mut self, mut watch_rx0 : watch::Receiver<i32>) {
        let mut pending_packet= Bytes::new();
        let mut out_buf = BytesMut::with_capacity(self.cfg.buf_size); 
        let mut out_packet = Bytes::new();; 
        
        loop {
            if pending_packet.remaining() > 0 && out_buf.len() < self.cfg.buf_size{
                out_buf.put(&mut pending_packet);
            }

            if out_packet.remaining() == 0 && out_buf.len() >= self.cfg.buf_size {
                out_packet = out_buf.split_to(self.cfg.buf_size).freeze();
            }

            select! {
                r = self.rx_data.recv(), if pending_packet.remaining() == 0 =>{
                    match r {
                        Some(ev) => {
                            match ev{
                                BcastEvent::Data { packet } => {
                                    self.ibytes += packet.remaining() as u64;
                                    pending_packet = packet;
                                    //out_buf = buf;
                                },
                            }
                        },
                        None => {
                            error!("recv broadcast data but got None");
                            break;
                        },
                    }
                }

                r = self.wr.write_buf(&mut out_packet), if out_packet.remaining() > 0 =>{
                    match r{
                        Ok(n) =>{
                            self.obytes += n as u64;
                        },
                        Err(_) => {
                            //error!("write socket error [{}]", e);
                            break;
                        },
                    }
                }

                _ = watch_rx0.changed() => {
                    if *watch_rx0.borrow() > 0 {
                        break;
                    } else {
                        continue;
                    }
                }
            }
        }

        // disable more packets coming in
        self.rx_data.close(); 

        //info!("pid {}: dlink run done", self.pid);
    }

}

async fn session_run<'a>(pid:u32, mut socket : TcpStream, cfg : Arc<Config>, tx: mpsc::Sender<SessionEvent>, mut rx_data : mpsc::Receiver<BcastEvent>, hub : &'a Arc<Hub>) {

    let (watch_tx, watch_rx) = watch::channel(0);
    
    let (rd, wr) = socket.split();
    let mut ulink = ULinkHalf::news(&cfg, pid, rd, hub);
    let mut dlink = DLinkHalf::news(&cfg, pid, wr, &mut rx_data);

    {
        let action_u = ulink.run(watch_rx.clone());
        let action_d = dlink.run(watch_rx.clone());

        let mut done_u = false;
        let mut done_d = false;    

        tokio::pin!(action_u);
        tokio::pin!(action_d);

        select! {
            _ = &mut action_u =>{done_u = true;}
            _ = &mut action_d =>{done_d = true;}
        }

        let _ = watch_tx.send(1);

        if !done_d {
            action_d.await;
        } 

        if !done_u{
            action_u.await;
        }

    }

    let _=tx.send(SessionEvent::Xfer{ts:xrs::time::now_millis(), ibytes:ulink.ibytes, obytes:dlink.obytes}).await;
    let _ = tx.send(SessionEvent::Finished{pid}).await;
}


fn spawn_tasks_hub(pid:u32, socket : TcpStream, cfg0 : &Arc<Config>, tx0: &mpsc::Sender<SessionEvent>, rx_data : mpsc::Receiver<BcastEvent>, hub : Arc<Hub>){
    let cfg = cfg0.clone();
    let tx = tx0.clone();
    tokio::spawn(async move {
        session_run(pid, socket, cfg, tx, rx_data, &hub).await;
        hub.remove(pid).await;
        //info!("pid {}: aaa final", pid);
        
    });
}

async fn run_me(cfg0 : Arc<Config>) -> core::result::Result<(), Box<dyn std::error::Error>>{
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
                            let (tx, rx) = mpsc::channel(16);
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


async fn test_mpsc_throughput(){
    enum ChEvent {
        Data { packet : Bytes},
        Length { packet_len : usize},
    }

    xrs::tracing_subscriber::init_simple_milli();

    let packet_len = 51200;
    let max_packets:u64 = 1*1000*1000*2 ;

    // let mut packet = Cursor::new(vec![0u8; packet_len]);
    //let mut buf = BytesMut::with_capacity(packet_len);
    let packet = Bytes::from(vec![0u8; packet_len]);

    let (tx, mut rx) = mpsc::channel(16);
    let handle= tokio::spawn(async move {
        let mut npkt:u64 = 0;
        let time = Instant::now();
        while npkt < max_packets {
            let pkt = rx.recv().await;
            if pkt.is_none(){
                break;
            }
            npkt +=1;
        }
        let ms = time.elapsed().as_millis() as u64;
        info!("recv packets {}, elapsed {} ms, {} q/s, {} MB/s",
            npkt, ms, 
            1000*npkt as u64/ms,
            1000*npkt*packet_len as u64/ms/1000/1000);
    });

    let mut npkt = 0;
    let time = Instant::now();
    while npkt < max_packets {
        // let r = tx.send(ChEvent::Data{packet:packet.clone()}).await;
        let r = tx.send(ChEvent::Length{packet_len}).await;
        if r.is_err(){
            break;
        }
        npkt +=1;
    }
    let ms = time.elapsed().as_millis() as u64;
    info!("send packets {}, elapsed {} ms, {} q/s, {} MB/s",
            npkt, ms, 
            1000*npkt as u64/ms,
            1000*npkt*packet_len as u64/ms/1000/1000);
    let _ = handle.await;
    exit(0);
}


//#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
#[tokio::main]
pub async fn main()  {
    //test_mpsc_throughput().await;

    xrs::tracing_subscriber::init_simple_milli();

    let cfg0 = Config::parse();
    info!("cfg={:?}", cfg0);
    if cfg0.length > cfg0.buf_size {
        error!("packet lenght large than buffer size");
        <Config as clap::IntoApp>::into_app().print_help().unwrap();
        return ;
    }
    
    let _ = run_me(Arc::new(cfg0)).await;
}

