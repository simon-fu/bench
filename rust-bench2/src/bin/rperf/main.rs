
/*
    - case1: when packet length = 128k (default) 
        iperf3: cpu 50%, bandwidth 12Gb/s
        tokio : cpu 80%, bandwidth 1.8Gb/s 
        diff  : 1.8/12=15%, or 12/1.8=6.66x

    - case2: when packet length = 1000
        iperf3: cpu 100%, bandwidth 3Gb/s
        tokio : cpu 100%, bandwidth 760Mb/s 
        diff  : 0.760/3=25%, or 3/0.760=3.94x
*/

#![feature(type_alias_impl_trait)]

use anyhow::{Result, bail};
use args::Args;
use clap::Parser;
use tracing::{info, error};

mod args;
mod packet;
pub mod async_rt;
mod impl_async;
mod impl_std;




// #[tokio::main]
fn main() -> Result<()> {
    let args = Args::parse(); 

    tracing_subscriber::fmt()
    .without_time()
    .with_target(false)
    .init();

    // info!("rust of iperf");
    info!("num_cpus: {}", num_cpus::get());

    let r = run_me(&args);
    match r {
        Ok(_r) => Ok(()),
        Err(e) => {
            error!("{:?}", e);
            Ok(())
        },
    }
}

fn run_me(args: &Args) -> Result<()> { 

    let client_args = args.client()?;
    let server_args = args.server()?;

    if client_args.is_some() && server_args.is_some() {
        bail!("cannot be both server and client");

    } else if let Some(client_args) = client_args {
        match args.runtime {
            args::RuntimeType::Std => impl_std::run_as_client(&client_args),
            args::RuntimeType::Tokio => impl_async::run_tokio_client(client_args),
        }

    } else if let Some(server_args) = server_args {
        match args.runtime {
            args::RuntimeType::Std => impl_std::run_as_server(&server_args),
            args::RuntimeType::Tokio => impl_async::run_tokio_server(server_args),
        }
        
    } else {
        bail!("parameter error - must either be a client (-c) or server (-s)");
    }
}







