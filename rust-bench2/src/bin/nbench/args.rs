

use clap::{Parser, Subcommand};
use anyhow::{Result, Context};
use rust_bench::util::normalize_addr;

#[derive(Parser, Debug, Clone)]
#[clap(name = "nbench", author, about = "network bench", long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Clone)]
#[derive(Subcommand)]
pub enum Commands {
    Client(ClientArgs),
    Server(ServerArgs),
}

#[derive(Parser, Debug, Clone)]
#[clap(about = "run as client")]
pub struct ClientArgs {
    #[clap(short = 'c', long = "target", long_help = "target server address to connect, in the format of ip:port")]
    pub target: String,

    #[clap(long = "bind", long_help = "bind to local address")]
    pub bind: Option<String>,

    #[clap(short = 'l', long = "len", long_help = "packet length; default 128 KB")]
    len: Option<usize>,

    #[clap(short = 'R', long = "reverse", long_help = "run in reverse mode (server sends, client receives)")]
    pub is_reverse: bool,

    #[clap(short = 't', long = "time", long_help = "time in seconds to transmit for", default_value = "10")]
    pub secs: u64,

    #[clap(long = "rt", long_help = "runtime", default_value = "tokio")]
    #[arg(value_enum)]
    pub runtime: RuntimeType,

    #[clap(long = "conn", long_help = "all connections to setup", default_value = "1")]
    pub conns: usize,

    #[clap(long = "cps", long_help = "setup connections rate", default_value = "1000")]
    pub cps: usize,
}


impl ClientArgs {
    pub fn normalize(&mut self) -> Result<()> { 
        normalize_addr(&mut self.target, DEFAULT_SERVER_PORT)
        .with_context(||"invalid target")?;

        if let Some(r) = &mut self.bind {
            normalize_addr(r, "0")
            .with_context(||"invalid bind")?;
        }
        
        Ok(())
    }

    pub fn packet_len(&self) -> usize {
        self.len.unwrap_or_else(||DEFAULT_PACKET_LEN)
    }
}

#[derive(Parser, Debug, Clone)]
#[clap(about = "run as server")]
pub struct ServerArgs {
    #[clap(long = "bind", long_help = "bind to local address", default_value = DEFAULT_BIND, )]
    pub bind: String,

    #[clap(long = "rt", long_help = "runtime", default_value = "tokio")]
    #[arg(value_enum)]
    pub runtime: RuntimeType,
}

impl ServerArgs {
    pub fn normalize(&mut self) -> Result<()>{
        normalize_addr(&mut self.bind, "0")
        .with_context(||"invalid bind")?;
        Ok(())
    }
}



pub const DEFAULT_SERVER_PORT: &str = "6111";
pub const DEFAULT_BIND: &str = "0.0.0.0:6111";
pub const DEFAULT_PACKET_LEN: usize = 128_000;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum RuntimeType {
    Std,
    Tokio,
}


