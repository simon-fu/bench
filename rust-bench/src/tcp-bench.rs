// refer https://github.com/haraldh/rust_echo_bench

use std::env;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use std::collections::VecDeque;

use tokio::io::Result;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time;
use tokio::time::Instant;

use bytes::{Buf, BytesMut};

//pub type Error = Box<dyn std::error::Error + Send + Sync>;
// pub type Result<T> = std::result::Result<T, Error>;

// from https://www.jianshu.com/p/e30eef29f66e
fn timestamp1() -> i64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let ms = since_the_epoch.as_secs() as i64 * 1000i64 + (since_the_epoch.subsec_nanos() as f64 / 1_000_000.0) as i64;
    ms
}

fn print_usage(_: &str, opts: &getopts::Options) {
    print!("{}", opts.usage(&""));
}


#[derive(Debug)]
struct Config{
    length: usize, duration: u64, number: u32, address: String, speed:u32
}
impl Default for Config {
    fn default() -> Config {
        Config {
            length: 0, duration: 0, number: 0, address: String::from(""), speed:0
        }
    }
}

#[derive(Debug)]
struct Count {
    inb: u64,
    outb: u64,
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
struct TsI64{
    ts : i64,
    num : i64,
}

#[derive(Debug)]
struct SpeedState{
    history : VecDeque<TsI64>, // ts, num
    sum : i64,
}

impl SpeedState{
    fn add(self: &mut Self, ts : i64, num : i64){
        self.history.push_back(TsI64{ts, num});
        self.sum += num;
    }

    fn cap(self : &mut Self, duration : i64){
        while !self.history.is_empty() {
            let d = self.history.back().unwrap().ts - self.history.front().unwrap().ts;
            if d > duration {
                self.sum -= self.history.front().unwrap().num;
                self.history.pop_front();
            } else {
                break;
            }
        }

        if !self.history.is_empty() {
            let &ts1 = &self.history.back().unwrap().ts;
            let &ts2 = &self.history.front().unwrap().ts;
            if ts1 == ts2 && ts1 > duration {
                self.history.push_front(TsI64{ts:ts1-duration, num:0});
            }
        }

    }

    fn average(self : &mut Self) -> i64{
        if self.history.is_empty() {
            0
        } else {
            let d = self.history.back().unwrap().ts - self.history.front().unwrap().ts;
            if d > 0 {1000 * (self.sum as i64) / d}
            else {0}
        }
    }
}

impl Default for SpeedState {
    fn default() -> SpeedState {
        SpeedState {
            sum : 0,
            history : VecDeque::new()
        }
    }
}

#[derive(Debug)]
enum Event {
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
        count: Count,
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
struct State{
    config : Arc<Config> ,
    xfer : Count,
    last_xfer_ts : i64,
    conn_ok_count : u32,
    conn_fail_count : u32,
    conn_broken_count : u32,
    finish_session_count : u32,
    conn_speed_state : SpeedState,
    max_conn_speed : i64,
    start_time : i64,
    deadline : Instant,
    finished : bool,
}


impl  State {
    fn new(cfg : &Arc<Config>) -> State {
        let now = timestamp1();
        State {
            config : cfg.clone(),
            xfer : Count::default(),
            last_xfer_ts : now,
            conn_ok_count : 0,
            conn_fail_count : 0,
            conn_broken_count : 0,
            finish_session_count : 0,
            conn_speed_state : SpeedState::default(),
            max_conn_speed : 0,
            start_time : now,
            deadline : Instant::now() + Duration::from_secs(cfg.duration),
            finished : false,
        }
    }

    fn process_ev(self: &mut Self, ev : &Event) -> bool{
        match ev {
            Event::ConnectOk { ts } => {
                self.conn_ok_count += 1;
                self.conn_speed_state.add(*ts, 1);
            }

            Event::ConnectFail { .. } => {
                self.conn_fail_count += 1;
            }

            Event::ConnectBroken { .. } => {
                self.conn_broken_count += 1;
            }

            Event::Xfer { ts, count } => {
                // /println!("got xfer {:?}", *count);
                self.xfer.inb += count.inb;
                self.xfer.outb += count.outb;
                if self.last_xfer_ts < *ts {
                    self.last_xfer_ts = *ts;
                }
            }

            Event::Finish { .. } => {
                self.finish_session_count +=1;
                return self.check_finished();
            }
            
        };
        return false;
    }

    fn check_finished(self: &mut Self) -> bool{
        if !self.finished {
            if Instant::now()>=self.deadline{
                self.print_progress();
                println!("reach duration {} sec", self.config.duration);
                self.finished = true;
            } else if self.is_sessison_finished() {
                self.print_progress();
                println!("all sessions finished ");
                self.finished = true;
            }

            if self.finished{

            }
        }
        return self.finished;
    }

    fn is_finished(self: &Self) -> bool{
        return self.finished;
    }

    fn is_sessison_finished(self: &Self) -> bool{
        return self.finish_session_count >= self.config.number;
    }

    fn print_progress(self: &mut Self){
        self.conn_speed_state.cap(2000);
        let average = self.conn_speed_state.average();
        if average > self.max_conn_speed {
            self.max_conn_speed = average;
        }
        println!("Connections: ok {}, fail {}, broken {}, total {}, average {} c/s, max {} c/s", 
            self.conn_ok_count, self.conn_fail_count, self.conn_broken_count, self.config.number, average, self.max_conn_speed
        );
    }

    fn print_final(self: &Self){
        println!("");
        println!("");
        
        println!("Sessions: finished {}, total {}", self.finish_session_count, self.config.number);
        let duration = self.last_xfer_ts - self.start_time;
        println!("Connections: success {}, fail {}, broken {}, total {}, max {} c/s", 
            self.conn_ok_count, self.conn_fail_count, self.conn_broken_count, self.config.number, self.max_conn_speed
        );

        println!( "Requests : total {},  average {} q/s", self.xfer.outb, 
            if duration > 0 {1000*self.xfer.outb as i64 / duration} else {0});
        println!( "Responses: total {},  average {} q/s", self.xfer.inb, 
            if duration > 0 {1000*self.xfer.inb as i64 / duration} else {0});
    }
}




async fn check_event(
    _cfg : &Arc<Config>, 
    rx: & mut mpsc::Receiver<Event>,  
    state: & mut State, 
    next_print_time : & mut Instant) -> bool{

    if state.is_finished(){
        return true;
    }

    tokio::select! {
        _ = time::sleep_until(*next_print_time) => {
            state.print_progress();
            *next_print_time = Instant::now() + Duration::from_millis(1000);
        }

        _ = time::sleep_until(state.deadline) => {
            if !state.is_finished() {
                if state.check_finished(){

                }
            }
            return state.is_finished();
            //break;
        }

        Some(ev) = rx.recv() => {
            //println!("GOT = {:?}", ev);
            if state.process_ev(&ev) {
                return true;
                //break;
            }
        }

        else => {
            //break;
        }
    };
    return false;
}







struct SessionState{
    stream : Option<TcpStream>,
    out_buf : Cursor<Vec<u8>>,
    in_buf : BytesMut,     
}

struct Session{
    cfg0 : Arc<Config>,
    tx0 : mpsc::Sender<Event>, 
    state : SessionState,
}

impl Session {
    fn new(cfg : &Arc<Config>, tx : &mpsc::Sender<Event>) -> Session { 
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

async fn session_read<'a>(rd : &mut tokio::net::tcp::ReadHalf<'a>, in_buf:&mut BytesMut, capcity:usize, inb: &mut u64) -> Result<usize>{
    loop{
        if in_buf.len() == capcity {
            unsafe {
                in_buf.set_len(0);
            }
            *inb += 1;
        }
        rd.read_buf(in_buf).await?;
    }

}

async fn session_write<'a>(wr : &mut tokio::net::tcp::WriteHalf<'a>, out_buf:&mut Cursor<Vec<u8>>, outb: &mut u64) -> Result<usize>{
    loop{
        if out_buf.remaining() == 0 {
            out_buf.set_position(0);
            *outb += 1;
        }
        wr.write_buf(out_buf).await?;
    }
}

async fn session_xfer(session : &mut Session, count: &mut Count) -> Result<usize>{
    let (mut rd, mut wr) = session.state.stream.as_mut().unwrap().split();

    let read_action = session_read(&mut rd, &mut session.state.in_buf, session.cfg0.length, &mut count.inb);
    let write_action = session_write(&mut wr, &mut session.state.out_buf, &mut count.outb);

    tokio::pin!(read_action);
    tokio::pin!(write_action);

    return tokio::select! {
        Err(e) = &mut write_action => { Err(e) }

        Err(e) = &mut read_action => { Err(e) }
    };
}

async fn session_connect(session : &mut Session) -> Result<()>{
    let conn_result = TcpStream::connect(&session.cfg0.address).await;
    match conn_result {
        Ok(s)=>{
            session.state.stream = Some(s);
            let _ = session.tx0.send(Event::ConnectOk { ts: timestamp1() }).await;
            return Ok(());
        }
        Err(e) => {
            println!("connect fail with [{}]", e);
            let _ = session.tx0.send(Event::ConnectFail { ts: timestamp1() }).await;
            return Err(e);
        } 
    }
}


async fn session_entry(mut session : Session, mut watch_rx0 : watch::Receiver<HubEvent>){
    let mut watch_state = *watch_rx0.borrow();
    let mut count = Count::default();

    loop {
        // conncting
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
        {
            let action = session_xfer(&mut session, &mut count);
            tokio::pin!(action);

            while matches!(watch_state, HubEvent::KickXfer) {
                tokio::select! {
                    Err(e) = &mut action => { 
                        println!("xfering but broken with error [{}]", e);
                        is_broken = true;
                        
                        break;
                    }
    
                    _ = session_watch(&mut watch_rx0, &mut watch_state) =>{ }
                }
            }
        }

        if is_broken {
            let _ = session.tx0.send(Event::ConnectBroken { ts: timestamp1() }).await;
        }

        let _ = session.tx0.send(Event::Xfer { ts: timestamp1(), count }).await;

        break;
    }

    // finally 
    let _ = session.tx0.send(Event::Finish { ts: timestamp1()}).await;
}


async fn bench(cfg : Arc<Config>){

    println!("spawn {} task...", cfg.number);
    
    let (tx, mut rx) = mpsc::channel(1024);
    let (watch_tx, watch_rx) = watch::channel(HubEvent::Ready);

    let kick_time = timestamp1();
    let mut state = State::new(&cfg);
    // let deadline = Instant::now() + Duration::from_secs(cfg.duration);
    let mut next_print_time = Instant::now() + Duration::from_millis(1000);

    for n in 0..cfg.number {
        let session = Session::new(&cfg, &tx);
        let watch_rx0 = watch_rx.clone();
        tokio::spawn(async move{
            session_entry(session, watch_rx0).await;
        });

        loop {
            let elapsed = timestamp1() - kick_time;
            let expect = 1000 * n / cfg.speed;
            let diff = expect as i64 - elapsed;
            if diff <= 0{
                break;
            }
            
            let action = check_event(&cfg, & mut rx, & mut state, &mut next_print_time);
            tokio::select! {
                _ = time::sleep(Duration::from_millis(diff as u64)) => {
                    break;
                }
                done = action =>{
                    if done {
                        break;
                    }
                }
            };
        }
        if state.is_finished(){
            break;
        }

    }
    drop(tx);
    drop(watch_rx);
    println!("spawn {} task done", cfg.number);

    if cfg.length > 0 {
        let _ = watch_tx.send(HubEvent::KickXfer);
    }

    while !state.is_finished() {
        check_event(&cfg, & mut rx, & mut state, &mut next_print_time).await;
    }

    let dead_time = Instant::now() + Duration::from_secs(10);
    while !state.is_sessison_finished(){
        let _ = watch_tx.send(HubEvent::ExitReq);
        tokio::select! {
            _ = time::sleep_until(dead_time) => {
                println!("waiting for task timeout");
                break;
            }
    
            Some(ev) = rx.recv() => {
                state.process_ev(&ev);
            }
    
            else => {
                break;
            }
        };
    }

    state.print_final();

    drop(rx);
}

async fn func1() -> i32 { 12 }

async fn func2() -> i32{
    let t = 1;                  
    let v = t + 1; 
    let b = func1().await;
    let rv = &v;   
    *rv + b
}

async fn func3() -> i32{
    let t = 1;                  
    let v = t + 1; 
    let rv = &v;   
    let b = func1().await;
    *rv + b
}

#[tokio::main]
pub async fn main() {
    let fut2 = func2();
    let fut3 = func3();
    println!("fut2 size: {}", std::mem::size_of_val(&fut2));
    println!("fut3 size: {}", std::mem::size_of_val(&fut3));

    let args: Vec<_> = env::args().collect();
    let program = args[0].clone();

    let mut opts = getopts::Options::new();
    opts.optflag("h", "help", "Print this help.");
    opts.optopt(
        "a",
        "address",
        "Target echo server address. Default: 127.0.0.1:7000",
        "<address>",
    );
    opts.optopt(
        "l",
        "length",
        "Test message length. Default: 512",
        "<length>",
    );
    opts.optopt(
        "t",
        "duration",
        "Test duration in seconds. Default: 60",
        "<duration>",
    );
    opts.optopt(
        "c",
        "number",
        "Test connection number. Default: 50",
        "<number>",
    );
    opts.optopt(
        "s",
        "speed",
        "setup connection speed. Default: 100",
        "<speed>",
    );

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            print_usage(&program, &opts);
            return;
        }
    };

    if matches.opt_present("h") {
        print_usage(&program, &opts);
        return;
    }

    
    let mut cfg = Config::default();
    cfg.length = matches
        .opt_str("length")
        .unwrap_or_default()
        .parse::<usize>()
        .unwrap_or(512);
    cfg.duration = matches
        .opt_str("duration")
        .unwrap_or_default()
        .parse::<u64>()
        .unwrap_or(60);
    cfg.number = matches
        .opt_str("number")
        .unwrap_or_default()
        .parse::<u32>()
        .unwrap_or(50);
    cfg.address = matches
        .opt_str("address")
        .unwrap_or_else(|| "127.0.0.1:7000".to_string());
    cfg.speed = matches
        .opt_str("speed")
        .unwrap_or_default()
        .parse::<u32>()
        .unwrap_or(100);


    println!("Benchmarking: {}", cfg.address);
    println!(
        "{} clients, {} c/s, running {} bytes, {} sec.",
        cfg.number, cfg.speed, cfg.length,  cfg.duration
    );
    println!();

    bench(Arc::new(cfg)).await;
}

// struct Count {
//     inb: u64,
//     outb: u64,
// }

// fn main0() {
//     let args: Vec<_> = env::args().collect();
//     let program = args[0].clone();

//     let mut opts = getopts::Options::new();
//     opts.optflag("h", "help", "Print this help.");
//     opts.optopt(
//         "a",
//         "address",
//         "Target echo server address. Default: 127.0.0.1:12345",
//         "<address>",
//     );
//     opts.optopt(
//         "l",
//         "length",
//         "Test message length. Default: 512",
//         "<length>",
//     );
//     opts.optopt(
//         "t",
//         "duration",
//         "Test duration in seconds. Default: 60",
//         "<duration>",
//     );
//     opts.optopt(
//         "c",
//         "number",
//         "Test connection number. Default: 50",
//         "<number>",
//     );

//     let matches = match opts.parse(&args[1..]) {
//         Ok(m) => m,
//         Err(f) => {
//             eprintln!("{}", f.to_string());
//             print_usage(&program, &opts);
//             return;
//         }
//     };

//     if matches.opt_present("h") {
//         print_usage(&program, &opts);
//         return;
//     }

//     let length = matches
//         .opt_str("length")
//         .unwrap_or_default()
//         .parse::<usize>()
//         .unwrap_or(512);
//     let duration = matches
//         .opt_str("duration")
//         .unwrap_or_default()
//         .parse::<u64>()
//         .unwrap_or(60);
//     let number = matches
//         .opt_str("number")
//         .unwrap_or_default()
//         .parse::<u32>()
//         .unwrap_or(50);
//     let address = matches
//         .opt_str("address")
//         .unwrap_or_else(|| "127.0.0.1:12345".to_string());

//     let (tx, rx) = mpsc::channel();

//     let stop = Arc::new(AtomicBool::new(false));
//     let control = Arc::downgrade(&stop);

//     for _ in 0..number {
//         let tx = tx.clone();
//         let address = address.clone();
//         let stop = stop.clone();
//         let length = length;

//         thread::spawn(move || {
//             let mut sum = Count { inb: 0, outb: 0 };
//             let mut out_buf: Vec<u8> = vec![0; length];
//             out_buf[length - 1] = b'\n';
//             let mut in_buf: Vec<u8> = vec![0; length];
//             let mut stream = TcpStream::connect(&*address).unwrap();

//             loop {
//                 if (*stop).load(Ordering::Relaxed) {
//                     break;
//                 }

//                 match stream.write_all(&out_buf) {
//                     Err(_) => {
//                         println!("Write error!");
//                         break;
//                     }
//                     Ok(_) => sum.outb += 1,
//                 }

//                 if (*stop).load(Ordering::Relaxed) {
//                     break;
//                 }

//                 match stream.read(&mut in_buf) {
//                     Err(_) => break,
//                     Ok(m) => {
//                         if m == 0 || m != length {
//                             println!("Read error! length={}", m);
//                             break;
//                         }
//                     }
//                 };
//                 sum.inb += 1;
//             }
//             tx.send(sum).unwrap();
//         });
//     }

//     thread::sleep(Duration::from_secs(duration));

//     match control.upgrade() {
//         Some(stop) => (*stop).store(true, Ordering::Relaxed),
//         None => println!("Sorry, but all threads died already."),
//     }

//     let mut sum = Count { inb: 0, outb: 0 };
//     for _ in 0..number {
//         let c: Count = rx.recv().unwrap();
//         sum.inb += c.inb;
//         sum.outb += c.outb;
//     }
//     println!("Benchmarking: {}", address);
//     println!(
//         "{} clients, running {} bytes, {} sec.",
//         number, length, duration
//     );
//     println!();
//     println!(
//         "Speed: {} request/sec, {} response/sec",
//         sum.outb / duration,
//         sum.inb / duration
//     );
//     println!("Requests: {}", sum.outb);
//     println!("Responses: {}", sum.inb);
// }
