
/*    
    TODO:
    - support async std
    - support smol
    - support UDP
    - support interval
*/

#![feature(type_alias_impl_trait)]

use anyhow::Result;
use args::{Args, Commands};
use clap::Parser;
use rust_bench::util::log;
use tracing::{info, error};


mod args;
mod packet;
mod impl_async;
mod impl_std;



fn main() -> Result<()> {
    // prometheus_test2::run_main();

    let args = Args::parse(); 

    log::init()?;

    info!("num_cpus: {}", num_cpus::get());

    let r = run_me(args);
    match r {
        Ok(_r) => Ok(()),
        Err(e) => {
            error!("{:?}", e);
            Ok(())
        },
    }
}

fn run_me(args: Args) -> Result<()> { 

    match args.command {
        Commands::Client(mut client_args) => {
            client_args.normalize()?;
            info!("runtime: {:?}", client_args.runtime);
            match client_args.runtime {
                args::RuntimeType::Std => impl_std::run_as_client(&client_args),
                args::RuntimeType::Tokio => impl_async::run_tokio_client(client_args),
            }
        }
        Commands::Server(mut server_args) => {
            server_args.normalize()?;
            info!("runtime: {:?}", server_args.runtime);
            match server_args.runtime {
                args::RuntimeType::Std => impl_std::run_as_server(&server_args),
                args::RuntimeType::Tokio => impl_async::run_tokio_server(server_args),
            }
        },
    }

}




