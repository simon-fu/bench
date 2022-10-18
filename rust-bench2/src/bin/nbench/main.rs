
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
use tracing::{info, error};


mod args;
mod packet;
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
            match client_args.runtime {
                args::RuntimeType::Std => impl_std::run_as_client(&client_args),
                args::RuntimeType::Tokio => impl_async::run_tokio_client(client_args),
            }
        }
        Commands::Server(mut server_args) => {
            server_args.normalize()?;
            match server_args.runtime {
                args::RuntimeType::Std => impl_std::run_as_server(&server_args),
                args::RuntimeType::Tokio => impl_async::run_tokio_server(server_args),
            }
        },
    }

}







