// refer https://github.com/haraldh/rust_echo_bench

mod xrs;
use xrs::speed::Speed;

use std::fmt::Debug;
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::Result;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time;
use tokio::time::Instant;

use tracing::info;
use tracing::debug;
use tracing::error;

use bytes::{Buf, BytesMut};
use clap::{Clap};
use clap::IntoApp;



// refer https://github.com/clap-rs/clap/tree/master/clap_derive/examples
#[derive(Clap, Debug)]
#[clap(name="tcp bench", author, about, version)]
struct Config{
    #[clap(short='l', long, default_value = "0", long_about="Message length. If 0, skip send/recv message.")]
    length: usize, 

    #[clap(short='t', long, default_value = "60", long_about="Duration in seconds")]
    duration: u64, 

    #[clap(short='c', long, default_value = "50", long_about="Connection number")]
    number: u32, 

    #[clap(short='a', long, default_value = "127.0.0.1:7000", long_about="Target server address.")]
    address: String, 

    #[clap(short='s', long, default_value = "100", long_about="Setup connection speed")]
    speed:u32
}


#[derive(Debug)]
#[derive(Copy, Clone)]
struct Count {
    inb: u64,
    outb: u64,
}

impl Count {
    fn clear(& mut self) {
        self.inb = 0;
        self.outb = 0;
    }
}

impl Default for Count {
    fn default() -> Count {
        Count {
            inb: 0,
            outb: 0,
        }
    }
}

#[derive(Debug)]
#[derive(Copy, Clone)]
struct XferCount {
    packets: Count,
    bytes: Count,
}

impl XferCount {
    fn clear(& mut self) {
        self.packets.clear();
        self.bytes.clear();
    }
}

impl Default for XferCount {
    fn default() -> XferCount {
        XferCount {
            packets: Count::default(),
            bytes: Count::default(),
        }
    }
}


#[derive(Debug)]
enum SessionEvent {
    ConnectOk {
        ts: i64,
    },
    ConnectFail {
        ts: i64,
    },
    ConnectBroken {
        ts: i64,
    },
    Xfer{
        ts: i64,
        data: XferCount,
    },
    Finish{
        ts: i64,
    }
}

enum HubEvent {
    Ready,
    KickXfer,
    ExitReq,
}
impl Clone for HubEvent {
    fn clone(&self) -> Self{
        match self {
            HubEvent::Ready => HubEvent::Ready,
            HubEvent::KickXfer => HubEvent::KickXfer,
            HubEvent::ExitReq => HubEvent::ExitReq
        }
    }
}

impl Copy for HubEvent {}

#[derive(Debug)]
struct MasterState{
    config : Arc<Config> ,
    xfer: XferCount,
    last_xfer_ts : i64,
    conn_ok_count : u32,
    conn_fail_count : u32,
    conn_broken_count : u32,
    spawn_session_count : u32,
    finish_session_count : u32,
    conn_speed_state : Speed,
    max_conn_speed : i64,
    start_time : i64,
    finished : bool,
    conn_updated : bool,

    //xfer_speed : Speed,
    xfer_updated : bool,
}


impl  MasterState {
    fn new(cfg : &Arc<Config>) -> MasterState {
        let now_ms = xrs::time::now_millis();
        MasterState {
            config : cfg.clone(),
            xfer : XferCount::default(),
            last_xfer_ts : now_ms,
            conn_ok_count : 0,
            conn_fail_count : 0,
            conn_broken_count : 0,
            spawn_session_count : 0,
            finish_session_count : 0,
            conn_speed_state : Speed::default(),
            max_conn_speed : 0,
            start_time : now_ms,
            finished : false,
            conn_updated : false,

            //xfer_speed : Speed::default(),
            xfer_updated : false,
        }
    }

    fn process_ev(self: &mut Self, ev : &SessionEvent) -> bool{
        //trace!("process_ev {:?}", ev);
        match ev {
            SessionEvent::ConnectOk { ts } => {
                self.conn_updated = true;
                self.conn_ok_count += 1;
                self.conn_speed_state.add(*ts, 1);
            }

            SessionEvent::ConnectFail { .. } => {
                self.conn_updated = true;
                self.conn_fail_count += 1;
            }

            SessionEvent::ConnectBroken { .. } => {
                self.conn_updated = true;
                self.conn_broken_count += 1;
            }

            SessionEvent::Xfer { ts, data } => {
                //trace!("got xfer {:?}", *count);
                self.xfer_updated = true;
                self.xfer.packets.inb += data.packets.inb;
                self.xfer.packets.outb += data.packets.outb;
                self.xfer.bytes.inb += data.bytes.inb;
                self.xfer.bytes.outb += data.bytes.outb;
                if self.last_xfer_ts < *ts {
                    self.last_xfer_ts = *ts;
                }
            }

            SessionEvent::Finish { .. } => {
                self.finish_session_count +=1;
                if self.is_sessison_finished() {
                    self.print_progress();
                    info!("all sessions finished ");
                    self.finished = true;
                }
                return self.finished;
            }
            
        };
        return false;
    }


    fn is_sessison_finished(self: &Self) -> bool{
        return self.finish_session_count >= self.spawn_session_count;
    }

    fn is_connecting_finished(self: &Self) -> bool {
        return (self.conn_ok_count + self.conn_fail_count) >= self.spawn_session_count;
    }

    fn print_xfer_progress(self: &Self){
        let duration = self.last_xfer_ts - self.start_time;
        info!( "Transfer: requests {} ({} q/s, {} KB/s), response {} ({} q/s, {} KB/s)", 
            self.xfer.packets.outb, 
            if duration > 0 {1000*self.xfer.packets.outb as i64 / duration} else {0},
            if duration > 0 {1000*self.xfer.bytes.outb as i64 / duration / 1000} else {0},
            self.xfer.packets.inb, 
            if duration > 0 {1000*self.xfer.packets.inb as i64 / duration} else {0},
            if duration > 0 {1000*self.xfer.bytes.inb as i64 / duration / 1000} else {0},
            );
    }

    fn print_conn_progress(self: &mut Self){
        let average = self.conn_speed_state.cap_average(2000, xrs::time::now_millis());
        if average > self.max_conn_speed {
            self.max_conn_speed = average;
        }
        info!("Connections: ok {}, fail {}, broken {}, total {}, average {} c/s, max {} c/s", 
            self.conn_ok_count, self.conn_fail_count, self.conn_broken_count, self.config.number, average, self.max_conn_speed
        );
    }

    fn print_progress(self: &mut Self){
        
        if self.conn_updated {
            self.conn_updated = false;
            self.print_conn_progress();
        }

        if self.xfer_updated {
            self.xfer_updated = false;
            self.print_xfer_progress();
        }
    }

    fn print_final(self: &Self){
        info!("");
        info!("");
        
        info!("Tasks      : spawn {}, finished {}, total {}", self.spawn_session_count, self.finish_session_count, self.config.number);
        
        info!("Connections: ok {}, fail {}, broken {}, max {} c/s", 
            self.conn_ok_count, self.conn_fail_count, self.conn_broken_count, self.max_conn_speed
        );

        self.print_xfer_progress();
        // let duration = self.last_xfer_ts - self.start_time;
        // info!( "Requests : total {},  average {} q/s", self.xfer.packets.outb, 
        //     if duration > 0 {1000*self.xfer.packets.outb as i64 / duration} else {0});
        // info!( "Responses: total {},  average {} q/s", self.xfer.packets.inb, 
        //     if duration > 0 {1000*self.xfer.packets.inb as i64 / duration} else {0});
    }

    async fn check_event(
        self: &mut Self, 
        rx: & mut mpsc::Receiver<SessionEvent>,  
        next_print_time : & mut Instant,
        dead_line : & Instant,
        dead_line_msg : &str,
        ) -> bool{
    
        tokio::select! {
            _ = time::sleep_until(*next_print_time) => {
                self.print_progress();
                *next_print_time = Instant::now() + Duration::from_millis(1000);
            }
    
            _ = time::sleep_until(*dead_line) => {
                info!("{}", dead_line_msg);
                return true;
            }
    
            Some(ev) = rx.recv() => {
                let _ = self.process_ev(&ev); 
            }
    
            else => {
                return true; // something wrong
            }
        };
        return false; 
    }
}


struct SessionState{
    stream : Option<TcpStream>,
    out_buf : Cursor<Vec<u8>>,
    in_buf : BytesMut,     
}

struct Session{
    cfg0 : Arc<Config>,
    tx0 : mpsc::Sender<SessionEvent>, 
    state : SessionState,
}

impl Session {
    fn new(cfg : &Arc<Config>, tx : &mpsc::Sender<SessionEvent>) -> Session { 
        Session{
            cfg0: cfg.clone(),
            tx0: tx.clone(),
            state: SessionState{
                stream: None,
                out_buf: Cursor::new(vec![0; cfg.length]),
                in_buf : BytesMut::with_capacity(cfg.length),
            },
        }
    }
}

async fn session_watch(watch_rx0 : &mut watch::Receiver<HubEvent>, state : &mut HubEvent) {
    let result = watch_rx0.changed().await;
    match  result {
        Ok(_) => {
            *state = *watch_rx0.borrow();
        }

        Err(_) => {}
    }
}

async fn session_read<'a>(rd : &mut tokio::net::tcp::ReadHalf<'a>, in_buf:&mut BytesMut, capcity:usize, npackets: &mut u64, nbytes: &mut u64) -> Result<usize>{
    //info!("session reading...");

    let result = rd.read_buf(in_buf).await;
    match result {
        Ok(_) => {
            if in_buf.len() == capcity {
                unsafe {
                    in_buf.set_len(0);
                }
                *npackets += 1;
                *nbytes += capcity as u64;
            }
        },
        Err(_) => {},
    }
    //info!("session reading done");
    return result;
}

async fn session_write<'a>(wr : &mut tokio::net::tcp::WriteHalf<'a>, out_buf:&mut Cursor<Vec<u8>>, npackets: &mut u64, nbytes: &mut u64) -> Result<usize>{
    //info!("session writing...");

    let result =  wr.write_buf(out_buf).await;
    match result {
        Ok(_) => {
            if out_buf.remaining() == 0 {
                out_buf.set_position(0);
                *npackets += 1;
                *nbytes += out_buf.remaining() as u64;
            }
        },
        Err(_) => {},
    }

    //info!("session writing done");
    return result;
}

// async fn session_read_loop<'a>(rd : &mut tokio::net::tcp::ReadHalf<'a>, in_buf:&mut BytesMut, capcity:usize, inb: &mut u64) -> Result<usize>{
//     loop{
//         session_read(rd, in_buf, capcity, inb).await?;
//     }
// }

// async fn session_write_loop<'a>(wr : &mut tokio::net::tcp::WriteHalf<'a>, out_buf:&mut Cursor<Vec<u8>>, outb: &mut u64) -> Result<usize>{
//     loop{
//         session_write(wr, out_buf, outb).await?;
//     }
// }

async fn session_read_until<'a>(rd : &mut tokio::net::tcp::ReadHalf<'a>, in_buf:&mut BytesMut, capcity:usize, xfer:&mut XferCount) -> Result<usize>{
    while xfer.packets.inb < xfer.packets.outb{
        session_read(rd, in_buf, capcity, &mut xfer.packets.inb, &mut xfer.bytes.inb).await?;
    }
    return Ok(0);
}

async fn session_xfer(session : &mut Session, xfer: &mut XferCount) -> Result<usize>{
    let (mut rd, mut wr) = session.state.stream.as_mut().unwrap().split();

    // let read_action = session_read(&mut rd, &mut session.state.in_buf, session.cfg0.length, &mut xfer.packets.inb, &mut xfer.bytes.inb);
    // let write_action = session_write(&mut wr, &mut session.state.out_buf, &mut xfer.packets.outb, &mut xfer.bytes.outb);

    // tokio::pin!(read_action);
    // tokio::pin!(write_action);

    return tokio::select! {
        r = session_read(&mut rd, &mut session.state.in_buf, session.cfg0.length, &mut xfer.packets.inb, &mut xfer.bytes.inb) => { r }

        r = session_write(&mut wr, &mut session.state.out_buf, &mut xfer.packets.outb, &mut xfer.bytes.outb) => { r }
    };
}

async fn session_connect(session : &mut Session) -> Result<()>{
    let conn_result = TcpStream::connect(&session.cfg0.address).await;
    match conn_result {
        Ok(s)=>{
            session.state.stream = Some(s);
            let _ = session.tx0.send(SessionEvent::ConnectOk { ts: xrs::time::now_millis() }).await;
            return Ok(());
        }
        Err(e) => {
            debug!("connect fail with [{}]", e);
            let _ = session.tx0.send(SessionEvent::ConnectFail { ts: xrs::time::now_millis() }).await;
            return Err(e);
        } 
    }
}


async fn session_entry(mut session : Session, mut watch_rx0 : watch::Receiver<HubEvent>){
    let mut watch_state = *watch_rx0.borrow();
    let mut xfer = XferCount::default();

    loop {
        // connecting
        {
            let action = session_connect(&mut session);
            tokio::pin!(action);
    
            while  !matches!(watch_state, HubEvent::ExitReq) {
                tokio::select! {
                    _  = &mut action =>{
                        break;
                    }
    
                    _ = session_watch(&mut watch_rx0, &mut watch_state) => { }
                }
            }
        }

        if session.state.stream.is_none() {
            break;
        }

        // waiting for KickXfer
        while !matches!(watch_state, HubEvent::ExitReq) 
            && !matches!(watch_state, HubEvent::KickXfer)  {
            session_watch(&mut watch_rx0, &mut watch_state).await;
        }

        // xfer bytes
        let mut is_broken = false;
        if matches!(watch_state, HubEvent::KickXfer) {

            {
                // info!("session xfering...."); 

                //let action = session_xfer(&mut session, &mut count);
                //tokio::pin!(action);
                let mut next_time = Instant::now() + Duration::from_millis(1000);

                while matches!(watch_state, HubEvent::KickXfer) {
                    tokio::select! {
                        r = session_xfer(&mut session, &mut xfer) => { 
                            match r {
                                Ok(_) => {},
                                Err(e) => {
                                    error!("xfering but broken with error [{}]", e);
                                    is_broken = true;
                                    break;
                                },
                            }
                        }
        
                        _ = time::sleep_until(next_time) => {
                            next_time = Instant::now() + Duration::from_millis(1000);
                            let _ = session.tx0.send(SessionEvent::Xfer { ts: xrs::time::now_millis(), data:xfer}).await;
                            xfer.clear();
                        }
                        _ = session_watch(&mut watch_rx0, &mut watch_state) =>{ }
                    }
                }

                // info!("session xfering done");
            }

            {
                //info!("session reading remains {:?} ...", count);
                let deadline = Instant::now() + Duration::from_secs(5);
                let (mut rd, mut _wr) = session.state.stream.as_mut().unwrap().split();
                let read_action = session_read_until(&mut rd, &mut session.state.in_buf, session.cfg0.length, &mut xfer);
                tokio::pin!(read_action);

                while !is_broken {
                    tokio::select! {
                        result = &mut read_action => { 
                            match result{
                                Ok(_) => { },
                                Err(e) => {
                                    error!("reading remains but broken with error [{}]", e);
                                    is_broken = true;
                                },
                            }
                            break;
                        }
        
                        _ = time::sleep_until(deadline) => {
                            error!("reading remains timeout");
                            break;
                        }
                    }
                }
                //info!("session reading remains done");
            }
        }

        if is_broken {
            let _ = session.tx0.send(SessionEvent::ConnectBroken { ts: xrs::time::now_millis() }).await;
        }

        let _ = session.tx0.send(SessionEvent::Xfer { ts: xrs::time::now_millis(), data:xfer }).await;

        break;
    }

    // finally 
    let _ = session.tx0.send(SessionEvent::Finish { ts: xrs::time::now_millis()}).await;
}


async fn bench(cfg : Arc<Config>){

    info!("try spawn {} task...", cfg.number);
    
    let (tx, mut rx) = mpsc::channel(1024);
    let (watch_tx, watch_rx) = watch::channel(HubEvent::Ready);
    let mut state = MasterState::new(&cfg);
    
    let mut next_print_time = Instant::now() + Duration::from_millis(1000);
    let deadline = Instant::now() + Duration::from_secs(cfg.duration);
    let dead_line_msg = format!("reach duration {} sec", state.config.duration);

    let mut is_finished = false;
    let kick_time = Instant::now();
    
    for n in 0..cfg.number {
        let session = Session::new(&cfg, &tx);
        let watch_rx0 = watch_rx.clone();
        state.spawn_session_count += 1;

        tokio::spawn(async move{
            session_entry(session, watch_rx0).await;
        });

        {
            let master_action = state.check_event(& mut rx,  &mut next_print_time, &deadline, &dead_line_msg);
            tokio::pin!(master_action);
    
            loop {
                let expect = 1000 * n / cfg.speed;
                let diff = expect as i64 - kick_time.elapsed().as_millis() as i64;
                if diff <= 0{
                    break;
                }
                
                tokio::select! {
                    _ = time::sleep(Duration::from_millis(diff as u64)) => {
                        break;
                    }
                    true = &mut master_action =>{
                        is_finished = true;
                        break;
                    }
                };
            }
        }

        if is_finished{
            break;
        }
    }
    
    drop(tx);
    drop(watch_rx);

    {
        let deadline0 = Instant::now() + Duration::from_secs(10);
        let dead_line_msg0 = "waiting for connection-result timeout";
        while !state.is_connecting_finished() {
            let done = state.check_event(& mut rx,  &mut next_print_time, &deadline0, &dead_line_msg0).await;
            if done {
                is_finished = true;
                break;
            }
        }
        state.print_progress();
        info!("spawned {} task", state.spawn_session_count);
    }

    if cfg.length > 0 && !is_finished && state.conn_ok_count > 0{
        info!("broadcast kick-xfer");
        let _ = watch_tx.send(HubEvent::KickXfer);
    }

    if !is_finished && !state.is_sessison_finished() && state.conn_ok_count > 0{
        info!("waiting for {} sec", cfg.duration);
        while !is_finished && !state.is_sessison_finished(){
            is_finished = state.check_event(& mut rx,  &mut next_print_time, &deadline, &dead_line_msg).await;
        }
    }

    if !state.is_sessison_finished() {
        info!("broadcast exit-request");
        let _ = watch_tx.send(HubEvent::ExitReq);

        let deadline = Instant::now() + Duration::from_secs(10);
        let dead_line_msg = "waiting for task timeout";
        while !state.is_sessison_finished(){
            let done = state.check_event(& mut rx,  &mut next_print_time, &deadline, &dead_line_msg).await;
            if done {
                break;
            }
        }
    }

    state.print_final();

    drop(rx);
}




#[tokio::main]
pub async fn main() {
    xrs::tracing_subscriber::init_simple_milli();

    let cfg = Config::parse();

    {
        let addr = cfg.address.parse::<SocketAddr>();
        if addr.is_err() {            
            error!("invalid address [{}]\n", cfg.address);
            Config::into_app().print_help().unwrap();
            return;
        }
    }

    debug!("cfg={:?}", cfg);
    info!("Benchmarking: {}", cfg.address);
    info!(
        "{} clients, {} c/s, running {} bytes, {} sec.",
        cfg.number, cfg.speed, cfg.length,  cfg.duration
    );
    info!("");

    bench(Arc::new(cfg)).await;
}

