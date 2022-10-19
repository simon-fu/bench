
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
use tracing::{info, error, metadata::LevelFilter};


mod args;
mod packet;
mod impl_async;
mod impl_std;



fn main() -> Result<()> {

    let args = Args::parse(); 

    {
        use tracing_subscriber::{EnvFilter, fmt::{self, time::OffsetTime}, prelude::*};
        use time::macros::format_description;

        // see https://time-rs.github.io/book/api/format-description.html
        let offset = time::UtcOffset::current_local_offset()?;
        let timer = OffsetTime::new(offset, format_description!("[hour]:[minute]:[second]:[subsecond digits:3]"));

        let layer = fmt::layer()
        .with_target(false)
        .with_timer(timer);

        let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

        tracing_subscriber::registry()
        .with(layer)
        .with(filter)
        .init();
    }
    


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







