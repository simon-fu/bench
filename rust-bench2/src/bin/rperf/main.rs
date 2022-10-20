
/*
    Bench result:
    - case1: when packet length = 128k (default) 
        iperf3: cpu 41%, bandwidth 11.3Gb/s
        tokio : cpu 55%, bandwidth 11.3Gb/s 
        diff  : cpu +14%, bandwidth 0Gb/s%

    - case2: when packet length = 1000
        iperf3: cpu  68%, bandwidth 3.0Gb/s
        tokio : cpu 100%, bandwidth 4.1Gb/s 
        diff  : cpu +32%, bandwidth +1.1Gb/s
    
    TODO:
    - support async std
    - support smol
    - support UDP
    - support interval
*/

#![feature(type_alias_impl_trait)]

use anyhow::{Result, bail};
use args::Args;
use clap::Parser;
use rust_bench::util::log;
use tracing::{info, error};

mod args;
mod packet;
mod impl_async;
mod impl_std;




// #[tokio::main]
fn main() -> Result<()> {
    let args = Args::parse(); 
    log::init()?;

    // info!("rust of iperf");
    info!("num_cpus {}, runtime {:?}", num_cpus::get(), args.runtime);

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







