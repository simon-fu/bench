
use anyhow:: Result;
use tokio::{runtime::{Runtime, self}, net::{TcpStream, TcpListener}};

use crate::args::{ClientArgs, ServerArgs};

use super::{run_as_client, run_as_server};

pub fn run_tokio_client(args: ClientArgs) -> Result<()> {
    let rt = runtime::Builder::new_multi_thread()
    .enable_all()
    .build()?;
    // let rt  = Runtime::new()?;

    rt.block_on(async {
        tokio::spawn(async move {
            run_as_client::<TcpStream>(&args).await
        }).await?
    })?;

    Ok(())
}

pub fn run_tokio_server(args: ServerArgs) -> Result<()> {
    let rt  = Runtime::new()?;

    rt.block_on(async {
        tokio::spawn(async move {
            run_as_server::<TcpListener, TcpStream>(&args).await
        }).await?
    })?;

    Ok(())
}
