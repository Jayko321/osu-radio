pub(crate) mod config;
#[cfg(feature = "docs")]
pub(crate) mod docs;
pub(crate) mod error;
pub(crate) mod routes;
pub(crate) mod services;
pub(crate) mod state;
#[cfg(test)]
pub(crate) mod test_support;

use std::process::ExitCode;

use anyhow::{Context, Result};
use tokio::net::TcpListener;

use crate::{config::ServerConfig, state::AppState};

#[tokio::main]
async fn main() -> ExitCode {
    match serve().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn serve() -> Result<()> {
    let config = ServerConfig::from_env()?;
    let state = AppState::connect(&config.database_url).await?;

    let listener = TcpListener::bind(&config.address)
        .await
        .with_context(|| format!("Failed to bind `{}`", config.address))?;
    let address = listener
        .local_addr()
        .context("Failed to read the bound address")?;

    println!("osu-radio server listening on http://{address}");
    println!("GET http://{address}/api/beatmap-sets returns every stored beatmap set.");
    println!("GET http://{address}/api/user-data returns the stored settings and osu! folders.");
    #[cfg(feature = "docs")]
    println!(
        "Scalar API reference: http://{address}{}",
        docs::SCALAR_PATH
    );

    axum::serve(listener, routes::router(state))
        .await
        .context("The server stopped unexpectedly")
}
