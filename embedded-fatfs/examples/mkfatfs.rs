use std::env;

use embedded_fatfs::{format_volume, FormatVolumeOptions};
use embedded_io_adapters::tokio_1::FromTokio;
use tokio::fs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filename = env::args().nth(1).expect("image path expected");
    let file = fs::OpenOptions::new().read(true).write(true).open(&filename).await?;
    // Note: Don't use tokio::io::BufStream - it slows down performance
    format_volume(&mut FromTokio::new(file), FormatVolumeOptions::new()).await?;
    Ok(())
}
