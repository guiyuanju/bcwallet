mod network;
mod rpcclient;
mod types;
mod wallet;

use crate::{network::NetworkClient, rpcclient::LocalRpcClient, wallet::Wallet};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::env;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new wallet file
    NewWallet,
    /// Get balance of current address
    Balance,
}

fn main() -> Result<()> {
    let wallet_path = env::var("WALLET").unwrap_or("wallet.json".to_owned());
    let port = env::var("BTC_RPC_PORT").unwrap_or("18332".to_owned());
    let username = env::var("BTC_RPC_USER").unwrap_or("user".to_owned());
    let passwd = env::var("BTC_RPC_PASS").unwrap_or("passwd".to_owned());

    let cli = Cli::parse();

    match cli.command {
        Commands::NewWallet => {
            let mut wallet = Wallet::new();
            wallet.compute_key_addr();
            wallet.save(&wallet_path)?;
        }
        Commands::Balance => {
            let mut wallet = Wallet::new();
            wallet.load(&wallet_path)?;
            let addr = wallet.address()?;
            let client = LocalRpcClient::new(&port, &username, &passwd)?.watch_address(&[&addr])?;
            let balance = client.get_balance(&addr)?;

            println!("{} satoshi", balance);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn make_tx() {
        assert!(true);
    }
}
