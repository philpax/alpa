use serde::{Deserialize, Serialize};

mod config;
mod host;
mod util;
mod window;

#[derive(Serialize, Deserialize)]
struct WindowArgs {
    width: u32,
    height: u32,
    style: config::Style,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Some(args) = std::env::args().nth(1) {
        let args = serde_json::from_str(&args)?;
        window::main(&args).await
    } else {
        host::main().await
    }
}
