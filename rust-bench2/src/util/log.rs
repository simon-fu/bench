
use anyhow::Result;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{EnvFilter, fmt::{self, time::OffsetTime}, prelude::*};
use time::macros::format_description;

pub fn init() -> Result<()>{

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
    Ok(())
}
