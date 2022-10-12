
use std::str::FromStr;

use anyhow::{Result, Context, bail};
use clap::Parser;
use client::{ClientArgs, run_as_client};
use server::{ServerArgs, run_as_server};
use tracing::{info, error};
use tracing_subscriber::EnvFilter;

mod server;
mod client;
mod common;
mod packet;

#[derive(Parser, Debug, Clone)]
#[clap(name = "rperf", author, about)]
pub struct Args {
    #[clap(short = 's', long = "server", long_help = "run in server mode")]
    is_server: bool,
    // -s, --server              run in server mode

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

    // #[clap(short = 't', long = "time", long_help = "time in seconds to transmit for", default_value = "10")]
    // secs: u64,

    // #[clap(short = 'i', long = "interval", long_help = "seconds between periodic bandwidth reports", default_value = "1")]
    // interval: u64,
}

#[derive(Debug, Clone)]
pub struct CommonArgs {
    pub server_port: u16,
    pub bind: Option<String>,
    // pub interval: u64,
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
                // secs: self.secs,
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse(); 

    const FILTER: &str = "warn,rperf=info"; 

    let env_filter = match std::env::var_os(EnvFilter::DEFAULT_ENV) {
        Some(_v) => EnvFilter::from_default_env(),
        None => EnvFilter::from_str(FILTER)?,
    };

    tracing_subscriber::fmt()
    .with_env_filter(env_filter)
    .with_target(false)
    .init();

    info!("rust of iperf");

    let r = run_me(&args).await;
    match r {
        Ok(_r) => Ok(()),
        Err(e) => {
            error!("{:?}", e);
            Ok(())
        },
    }
}

async fn run_me(args: &Args) -> Result<()> { 

    let client_args = args.client()?;
    let server_args = args.server()?;

    if client_args.is_some() && server_args.is_some() {
        bail!("cannot be both server and client");
    } else if let Some(client_args) = client_args {
        run_as_client(&client_args).await
    } else if let Some(server_args) = server_args {
        run_as_server(&server_args).await
    } else {
        bail!("parameter error - must either be a client (-c) or server (-s)");
    }
}







