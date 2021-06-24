// refer https://github.com/haraldh/rust_echo_bench

mod xrs;
use xrs::speed::Speed;
use xrs::speed::Pacer;
use xrs::traffic;
use xrs::traffic::{Transfer};

use std::fmt::Debug;
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::u64;

use tokio::io::{Result, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio::time;
use tokio::time::Instant;

use tracing::{info, debug, error};

use bytes::{BytesMut};
use clap::{Clap, IntoApp};




// refer https://github.com/clap-rs/clap/tree/master/clap_derive/examples
#[derive(Clap, Debug)]
#[clap(name="tcp bench", author, about, version)]
struct Config{
    #[clap(short='l', long, default_value = "512", long_about="Packet length.")]
    length: u64, 

    #[clap(short='b', long="buffer", default_value = "0", long_about="buffer size. it is same as length if 0.")]
    buf_size: u64,

    #[clap(short='t', long, default_value = "60", long_about="Duration in seconds.")]
    duration: u64, 

    #[clap(short='c', long, default_value = "50", long_about="Connection number.")]
    connections: u64, 

    #[clap(short='a', long, default_value = "127.0.0.1:7000", long_about="Target server address.")]
    address: String, 

    #[clap(short='s', long, default_value = "100", long_about="Setup connection speed.")]
    speed:u32,

    #[clap(short='q', long, default_value = "0", long_about="packets/second for each connection.")]
    qps:u32,

    #[clap(short='p', long, default_value = "0", long_about="number of packets for each connection. if both of packets and qps are 0, disable sending.")]
    packets: u64, 
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
        data: Transfer,
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

const MASTER_CHECK_INTERVAL: u64 = 1000;


#[derive(Debug)]
struct MasterState{
    config : Arc<Config> ,
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

    xfer_updated : bool,
    xfer: Transfer,
    start_xfer_ts: i64,
    last_xfer_ts : i64,
    last_print_xfer : i64,
    send_speed : traffic::Speeds,
    recv_speed : traffic::Speeds,
}


impl  MasterState {
    fn new(cfg : &Arc<Config>) -> MasterState {
        let now_ms = xrs::time::now_millis();
        MasterState {
            config : cfg.clone(),
            
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

            xfer_updated : false,
            xfer : Transfer::default(),
            start_xfer_ts : now_ms,
            last_xfer_ts : now_ms,
            last_print_xfer : now_ms,
            send_speed : traffic::Speeds::default(),
            recv_speed : traffic::Speeds::default(),
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
                self.xfer += *data;
                self.send_speed.add(*ts, &data.output);
                self.recv_speed.add(*ts, &data.input);
                self.check_xfer(ts);
                if (self.xfer.output.packets > 0)
                    && (self.xfer.output.packets == (self.config.packets * self.config.connections))
                    && (self.xfer.input.packets >= self.xfer.output.packets) {
                    //let _ = watch_tx.send(HubEvent::ExitReq);
                    self.finished = true;
                    return self.finished;
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

    fn check_xfer(self: &mut Self, ts:&i64) {
        self.xfer_updated = true;
        if self.last_xfer_ts < *ts {
            self.last_xfer_ts = *ts;
        }

        if (xrs::time::now_millis() - self.last_print_xfer) >= MASTER_CHECK_INTERVAL as i64 {
            self.check_print_xfer();
        }
    }


    fn is_sessison_finished(self: &Self) -> bool{
        return self.finish_session_count >= self.spawn_session_count;
    }

    fn is_connecting_finished(self: &Self) -> bool {
        return (self.conn_ok_count + self.conn_fail_count) >= self.spawn_session_count;
    }

    fn print_xfer_progress(self: &mut Self){

        info!( "Send {} q, {} B ({} q/s, {} KB/s),   Recv {} q, {} B ({} q/s, {} KB/s)", 
            self.xfer.output.packets, self.xfer.output.bytes, 
            self.send_speed.packets.cap_average(2000, xrs::time::now_millis()),
            self.send_speed.bytes.cap_average(2000, xrs::time::now_millis())/1000,

            self.xfer.input.packets, self.xfer.input.bytes,
            self.recv_speed.packets.cap_average(2000, xrs::time::now_millis()),
            self.recv_speed.bytes.cap_average(2000, xrs::time::now_millis())/1000,
        );
    }

    fn check_print_xfer(self: &mut Self){
        if self.xfer_updated {
            self.xfer_updated = false;
            self.print_xfer_progress();
            self.last_print_xfer = xrs::time::now_millis();
        }
    }

    fn print_conn_progress(self: &mut Self){
        let average = self.conn_speed_state.cap_average(2000, xrs::time::now_millis());
        if average > self.max_conn_speed {
            self.max_conn_speed = average;
        }
        info!("Connections: ok {}, fail {}, broken {}, total {}, average {} c/s, max {} c/s", 
            self.conn_ok_count, self.conn_fail_count, self.conn_broken_count, self.config.connections, average, self.max_conn_speed
        );
    }

    fn print_progress(self: &mut Self){
        
        if self.conn_updated {
            self.conn_updated = false;
            self.print_conn_progress();
        }

        self.check_print_xfer();
    }

    fn print_final(self: &mut Self){
        info!("");
        info!("");
        
        info!("Tasks      : spawn {}, finished {}, total {}", self.spawn_session_count, self.finish_session_count, self.config.connections);
        
        info!("Connections: ok {}, fail {}, broken {}, max {} c/s", 
            self.conn_ok_count, self.conn_fail_count, self.conn_broken_count, self.max_conn_speed
        );

        info!("Transfer   : send {} packets, {} bytes,   recv {} packets, {} bytes", 
            self.xfer.output.packets, self.xfer.output.bytes, 
            self.xfer.input.packets, self.xfer.input.bytes,
        );

        //self.print_xfer_progress();
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
                *next_print_time = Instant::now() + Duration::from_millis(MASTER_CHECK_INTERVAL);
            }
    
            _ = time::sleep_until(*dead_line) => {
                info!("{}", dead_line_msg);
                return true;
            }
    
            Some(ev) = rx.recv() => {
                return self.process_ev(&ev); 
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
    output_bytes :  u64,
    input_bytes  :  u64,
}

impl Session {
    fn new(cfg : &Arc<Config>, tx : &mpsc::Sender<SessionEvent>) -> Session { 
        Session{
            cfg0: cfg.clone(),
            tx0: tx.clone(),
            state: SessionState{
                stream: None,
                out_buf: Cursor::new(vec![0; cfg.buf_size as usize]),
                in_buf : BytesMut::with_capacity(cfg.buf_size as usize),
            },
            output_bytes : 0,
            input_bytes : 0,
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


fn calc_report(
    packet_len : &u64,

    input_bytes : &u64, 
    output_bytes : &u64, 
    
    report_input_bytes : &mut u64, 
    report_output_bytes : &mut u64, 

    batch_xfer : &mut Transfer)  {
        
    batch_xfer.input.packets = (input_bytes - *report_input_bytes) / packet_len;
    batch_xfer.output.packets = (output_bytes - *report_output_bytes) / packet_len;

    batch_xfer.input.bytes = batch_xfer.input.packets * packet_len;
    batch_xfer.output.bytes = batch_xfer.output.packets * packet_len;

    *report_input_bytes += batch_xfer.input.bytes;
    *report_output_bytes += batch_xfer.output.bytes;
}

async fn session_xfer(session : &mut Session, watch_rx0 : &mut watch::Receiver<HubEvent>, watch_state:&mut HubEvent) -> Result<()>{    
    let mut batch_xfer = Transfer::default();

    let (mut rd, mut wr) = session.state.stream.as_mut().unwrap().split();
    let mut next_time = Instant::now() + Duration::from_millis(100);
    let mut result:Result<()> = Ok(());
    let writing_pacer = Pacer::new(session.cfg0.qps as u64);
    let enable_writing = session.cfg0.qps > 0 || session.cfg0.packets > 0;
    let max_output_bytes = session.cfg0.packets * session.cfg0.length;
    let mut report_output_bytes = 0u64;
    let mut report_input_bytes = 0u64;

    while matches!(watch_state, HubEvent::KickXfer) { 

        let remaining_bytes = max_output_bytes - session.output_bytes;

        if max_output_bytes > 0 && remaining_bytes == 0 && session.input_bytes == session.output_bytes {
            break;
        }

        if remaining_bytes < session.cfg0.buf_size {
            session.state.out_buf.set_position(session.cfg0.buf_size - remaining_bytes);
        } else {
            session.state.out_buf.set_position(0);
        }

        let wait_millis = writing_pacer.get_wait_milli(session.output_bytes/session.cfg0.length);
        
        let is_writing = enable_writing && remaining_bytes > 0 && wait_millis <= 0;

        tokio::select! {

            r = rd.read_buf(&mut session.state.in_buf) => {
                match r {
                    Ok(n) => {
                        if n == 0 {
                            result = Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "connection closed by peer"));
                            break;
                        }
                        session.input_bytes += n as u64;

                        unsafe {
                            session.state.in_buf.set_len(0);
                        }
                    },
                    Err(e) => {
                        error!("reading but broken with error [{}]", e);
                        result = Err(e);
                        break;
                    },
                }
            }

            r =  wr.write_buf(&mut session.state.out_buf), if is_writing =>{
                match r {
                    Ok(n) => {
                        session.output_bytes += n as u64;
                    },
                    Err(e) => {
                        error!("reading but broken with error [{}]", e);
                        result = Err(e);
                        break;
                    },
                }
            }


            _ = time::sleep(Duration::from_millis(wait_millis as u64)), if enable_writing && wait_millis > 0 =>{

            }

            _ = time::sleep_until(next_time) => {
                //info!("==== report: ibytes {}, obytes {}", session.input_bytes, session.output_bytes);
                calc_report(&session.cfg0.length, 
                    &session.input_bytes, &session.output_bytes,
                    &mut report_input_bytes, &mut report_output_bytes, &mut batch_xfer);

                if batch_xfer.input.packets > 0 || batch_xfer.output.packets > 0 {
                    // info!(" report {:?}", batch_xfer);
                    let _ = session.tx0.send(SessionEvent::Xfer { ts: xrs::time::now_millis(), data:batch_xfer}).await;
                    batch_xfer.clear();
                    
                }
                next_time = Instant::now() + Duration::from_millis(100);
            }

            _ = session_watch( watch_rx0, watch_state) =>{ }
        }
        
    }

    // if result.is_ok() && !matches!(watch_state, HubEvent::ExitReq) {
    //     result = session_read_remains(session, &mut batch_xfer).await;
    // }

    // info!("send last report");
    calc_report(&session.cfg0.length, 
        &session.input_bytes, &session.output_bytes,
        &mut report_input_bytes, &mut report_output_bytes, &mut batch_xfer);

    let _ = session.tx0.send(SessionEvent::Xfer { ts: xrs::time::now_millis(), data:batch_xfer }).await;
    batch_xfer.clear();

    return result;
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

        if matches!(watch_state, HubEvent::KickXfer) {
            let r = session_xfer(&mut session, &mut watch_rx0, &mut watch_state).await;
            if r.is_err() {
                let _ = session.tx0.send(SessionEvent::ConnectBroken { ts: xrs::time::now_millis() }).await;
                break;
            }
        }

        // waiting for ExitReq
        while !matches!(watch_state, HubEvent::ExitReq)  {
            session_watch(&mut watch_rx0, &mut watch_state).await;
        }

        break;
    }

    // finally 
    let _ = session.tx0.send(SessionEvent::Finish { ts: xrs::time::now_millis()}).await;
}


async fn bench(cfg : Arc<Config>){

    debug!("try spawn {} task...", cfg.connections);
    
    let (tx, mut rx) = mpsc::channel(1024);
    let (watch_tx, watch_rx) = watch::channel(HubEvent::Ready);
    let mut state = MasterState::new(&cfg);
    
    let mut next_print_time = Instant::now() + Duration::from_millis(MASTER_CHECK_INTERVAL);
    let deadline = Instant::now() + Duration::from_secs(cfg.duration);
    let dead_line_msg = format!("reach duration {} sec", state.config.duration);

    let mut is_finished = false;
    let kick_time = Instant::now();
    
    for n in 0..cfg.connections {
        let session = Session::new(&cfg, &tx);
        let watch_rx0 = watch_rx.clone();
        state.spawn_session_count += 1;

        tokio::spawn(async move{
            session_entry(session, watch_rx0).await;
        });
        
        loop {
            let expect = 1000 * n / cfg.speed as u64;
            let diff = expect as i64 - kick_time.elapsed().as_millis() as i64;
            if diff <= 0{
                break;
            }
            
            tokio::select! {
                _ = time::sleep(Duration::from_millis(diff as u64)) => {
                    break;
                }
                r = state.check_event(& mut rx,  &mut next_print_time, &deadline, &dead_line_msg) =>{
                    if r {
                        is_finished = true;
                        break;
                    }
                }
            };
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
        debug!("spawned {} task", state.spawn_session_count);
    }

    if !is_finished && state.conn_ok_count > 0{
        info!("press Enter to kick sending");
        let _ = std::io::Read::read(&mut std::io::stdin(), &mut [0u8]).unwrap();
        
        debug!("broadcast kick-xfer");
        state.start_xfer_ts = xrs::time::now_millis();
        let _ = watch_tx.send(HubEvent::KickXfer);
    }

    if !is_finished && !state.is_sessison_finished() && state.conn_ok_count > 0{
        debug!("waiting for {} sec", cfg.duration);
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

    let mut cfg = Config::parse();
    info!("cfg={:?}", cfg);

    if cfg.packets > 0 || cfg.qps > 0 {
        if cfg.packets == 0 {
            cfg.packets = std::u64::MAX/2 -1;
        }
    
        if cfg.qps == 0 {
            cfg.qps = std::u32::MAX/2 -1;
        }
    }

    if cfg.buf_size == 0 {
        cfg.buf_size = cfg.length;
    }

    if cfg.length > cfg.buf_size {
        error!("invalid packet lenght, it large than buffer size");
        <Config as clap::IntoApp>::into_app().print_help().unwrap();
        return ;
    }

    if cfg.length == 0 {
        error!("invalid packet lenght 0");
        <Config as clap::IntoApp>::into_app().print_help().unwrap();
        return ;
    }

    {
        let addr = cfg.address.parse::<SocketAddr>();
        if addr.is_err() {            
            error!("invalid address [{}]\n", cfg.address);
            Config::into_app().print_help().unwrap();
            return;
        }
    }

    
    info!("Benchmarking: {}", cfg.address);
    info!(
        "{} clients, {} c/s, running {} bytes, {} sec.",
        cfg.connections, cfg.speed, cfg.length,  cfg.duration
    );
    info!("");

    bench(Arc::new(cfg)).await;
}

