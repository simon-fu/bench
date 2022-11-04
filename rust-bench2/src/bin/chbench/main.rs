
#![feature(type_alias_impl_trait)]

use anyhow::Result;

pub mod event;
pub mod latency;

#[tokio::main]
async fn main() -> Result<()> { 
    event::bench().await?;
    
    Ok(())
}


