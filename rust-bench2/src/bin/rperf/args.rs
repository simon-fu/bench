
use anyhow::{Result, Context};
use clap::Parser;


#[derive(Parser, Debug, Clone)]
#[clap(name = "rperf", author, about)]
pub struct Args {

    #[clap(long = "rt", long_help = "runtime", default_value = "tokio")]
    #[arg(value_enum)]
    pub runtime: RuntimeType,

    #[clap(short = 's', long = "server", long_help = "run in server mode")]
    is_server: bool,

    #[clap(short = 'c', long = "client", long_help = "run in client mode, connecting to <host>")]
    host: Option<String>,

    #[clap(short = 'l', long = "len", long_help = "length of buffer to read or write; default 128 KB for TCP, 8K for UDP")]
    len: Option<usize>,

    #[clap(short = 'R', long = "reverse", long_help = "run in reverse mode (server sends, client receives)")]
    is_reverse: bool,

    #[clap(short = 'p', long = "port", long_help = "server port to listen on/connect to", default_value = "5201")]
    port: u16,

    #[clap(short = 'B', long = "bind", long_help = "bind to a specific interface")]
    bind: Option<String>,

    #[clap(long = "cport", long_help = "bind to a specific client port (TCP and UDP, default: ephemeral port)")]
    cport: Option<u16>,

    #[clap(short = 't', long = "time", long_help = "time in seconds to transmit for", default_value = "10")]
    secs: u64,

    // #[clap(short = 'i', long = "interval", long_help = "seconds between periodic bandwidth reports", default_value = "1")]
    // interval: u64,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum RuntimeType {
    Std,
    Tokio,
}

#[derive(Debug, Clone)]
pub struct CommonArgs {
    pub server_port: u16,
    pub bind: Option<String>,
    // pub interval: u64,
}

#[derive(Debug, Clone)]
pub struct ServerArgs {
    pub common: CommonArgs
}

#[derive(Debug, Clone)]
pub struct ClientArgs {
    pub common: CommonArgs,
    pub host: String,
    pub len: usize,
    pub is_reverse: bool,
    pub cport: u16,
    pub secs: u64,
}


impl Args {
    pub fn server(&self) -> Result<Option<ServerArgs>> {
        match self.is_server {
            true => Ok(Some(ServerArgs {
                common: self.common()?,
            })),
            false => Ok(None),
        }
    }

    pub fn client(&self) -> Result<Option<ClientArgs>> {
        match &self.host {
            Some(_) => Ok(Some(ClientArgs {
                common: self.common()?,
                host: self.host.as_ref().with_context(||"no host for client mod")?.clone(),
                is_reverse: self.is_reverse,
                len: self.len(),
                cport: self.cport.unwrap_or_else(||0),
                secs: self.secs,
            })),
            None => Ok(None),
        }
    }

    fn common(&self) -> Result<CommonArgs> {
        Ok(CommonArgs {
            server_port: self.port,
            bind: self.bind.clone(),
            // interval: self.interval,
        })
    }

    fn len(&self) -> usize {
        self.len.unwrap_or_else(||128_000)
    }
}
