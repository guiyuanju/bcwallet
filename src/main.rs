mod btc_client;
mod input_select_strategy;
mod transaction;
mod utils;
mod uxtoset;
mod wallet;

use crate::{
    btc_client::{BtcClient, localrpc::LocalRpc},
    utils::{as_hex, decode_base58},
    wallet::Wallet,
};
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
    /// Watch current address, need only called once for each new address
    Watch,
    /// Decode a base58 encoded string
    DecodeBase58 { src: String },
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
            let client = LocalRpc::new(&port, &username, &passwd)?;
            let balance = client.get_balance(&addr)?;

            println!("{}", balance);
        }
        Commands::Watch => {
            let mut wallet = Wallet::new();
            wallet.load(&wallet_path)?;
            let addr = wallet.address()?;
            let client = LocalRpc::new(&port, &username, &passwd)?;
            client.watch_address(&[&addr])?;
        }
        Commands::DecodeBase58 { src } => {
            let bytes = decode_base58(&src)?;
            let hex = as_hex(&bytes);
            println!("{:?}", hex);
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
