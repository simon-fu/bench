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


// refer https://github.com/clap-rs/clap/tree/master/clap_derive/examples
#[derive(Clap, Debug)]
#[clap(name="tcp echo server", author, about, version)]
struct Config{
    #[clap(short='a', long, default_value = "127.0.0.1:7000", long_about="listen address.")]
    address: String, 
}

enum SessionEvent {
    Finished { input_bytes:u32},
}


struct Server{
    session_count : u32,
    input_bytes : u64,
    conn_speed : Speed,
    updated : bool,

    max_conn_speed : i64,
}

impl Server{
    fn clear(self: &mut Self){
        self.session_count = 0;
        self.input_bytes = 0;
        self.updated = false;
        self.conn_speed.clear();
        self.max_conn_speed = 0;
    }

    fn add_session(self: &mut Self){
        self.conn_speed.add(xrs::time::now_millis(), 1);
        self.session_count += 1;
        self.updated = true;
    }

    fn check_print_sessions(self: &mut Self){
        if !self.updated {
            return;
        }
        self.updated = false;

        let average = self.conn_speed.cap_average(2000, xrs::time::now_millis());
        if average > self.max_conn_speed {
            self.max_conn_speed = average;
        }
        info!("Sessions: total {}, average {} c/s, max {} c/s", 
            self.session_count, average, self.max_conn_speed
        );

        if self.session_count == 0 {
            info!("input bytes {}", self.input_bytes);
            info!("no session exist, clear up");
            info!("");
            self.clear();
        }
    }

    fn process_ev(self: &mut Self, ev : &SessionEvent) {
        //trace!("process_ev {:?}", ev);
        match ev {
            SessionEvent::Finished { input_bytes } => {
                self.session_count -= 1;
                self.input_bytes += *input_bytes as u64;
                self.updated = true;
            }
        };
    }
    
}

async fn session_entry(mut socket : TcpStream, input_bytes:&mut u32){
    loop {
        let ready = socket.ready(Interest::READABLE).await.expect("fail to check socket ready state");

        if ready.is_readable() {
            let mut buf = vec![0; 1024];

            match socket.try_read(&mut buf) {
                Ok(n) => {
                    
                    if n == 0 {
                        // gracefully closed
                        return;
                    }
                    *input_bytes += n as u32;
    
                    let result = socket.write_all(&buf[0..n]).await;
                    match result {
                        Ok(_) => {},
                        Err(e) => {
                            debug!("failed to write data to socket, error=[{}]", e);
                            return;
                        },
                    }
                }

                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    debug!("failed to read data from socket, error=[{}]", e);
                    return;
                }
            }

        }
    }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    xrs::tracing_subscriber::init_simple_milli();

    let cfg = Config::parse();
    let listener = TcpListener::bind(&cfg.address).await?;
    info!("Listening on: {}", cfg.address);

    let mut serv = Server{ session_count: (0), conn_speed: Speed::default(), updated: (false), max_conn_speed: (0), input_bytes: (0) };
    let (tx, mut rx) = mpsc::channel(1024);
    let mut next_print_time = Instant::now() + Duration::from_millis(1000);

    loop {

        tokio::select! {
            result = listener.accept() => {
                match result{
                    Ok((socket, _)) => {
                        let tx0 = tx.clone();
                        serv.add_session();
                        tokio::spawn(async move {
                            let mut input_bytes:u32 = 0;
                            session_entry(socket, &mut input_bytes).await;
                            let _=tx0.send(SessionEvent::Finished { input_bytes }).await;
                        });
                    },
                    Err(_) => {}, 
                } 
            }

            _ = time::sleep_until(next_print_time) => {
                serv.check_print_sessions();
                next_print_time = Instant::now() + Duration::from_millis(1000);
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

