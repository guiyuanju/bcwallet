mod btc_client;
mod input_select_strategy;
mod params;
mod transaction;
mod utils;
mod utxoset;
mod wallet;

use crate::{
    btc_client::{BtcClient, localrpc::LocalRpc},
    params::{Receivers, TransactionParams},
    transaction::TransactionManager,
    utils::{as_hex, decode_base58},
    wallet::Wallet,
};
use anyhow::{Context, Result, bail};
use bitcoin::Network;
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
    /// Prepare transaction params file (online, requires RPC)
    Prepare {
        /// Receivers in "address:amount_sat" format (repeatable)
        #[arg(long, required = true)]
        receiver: Vec<String>,
        /// Output params file path
        #[arg(long, default_value = "params.json")]
        output: String,
    },
    /// Sign a transaction from params file (offline, no network)
    Sign {
        /// Path to params.json
        params: String,
    },
}

struct Config {
    network: Network,
    wallet_path: String,
    rpc_port: String,
    rpc_user: String,
    rpc_pass: String,
}

impl Config {
    fn from_env() -> Result<Self> {
        let network = match env::var("BTC_NETWORK").as_deref() {
            Ok("mainnet") | Ok("bitcoin") => Network::Bitcoin,
            Ok("signet") => Network::Signet,
            Ok("regtest") => Network::Regtest,
            Ok("testnet") | Err(_) => Network::Testnet,
            Ok(other) => bail!("unknown network: {other}"),
        };
        Ok(Self {
            network,
            wallet_path: env::var("WALLET").unwrap_or("wallet.json".to_owned()),
            rpc_port: env::var("BTC_RPC_PORT").unwrap_or("18332".to_owned()),
            rpc_user: env::var("BTC_RPC_USER").unwrap_or("user".to_owned()),
            rpc_pass: env::var("BTC_RPC_PASS").unwrap_or("passwd".to_owned()),
        })
    }

    fn rpc_client(&self) -> Result<LocalRpc> {
        LocalRpc::new(&self.rpc_port, &self.rpc_user, &self.rpc_pass)
    }

    fn wallet(&self) -> Result<Wallet> {
        Wallet::from_file(&self.wallet_path, self.network)
    }
}

fn main() -> Result<()> {
    let cfg = Config::from_env()?;
    let cli = Cli::parse();

    match cli.command {
        Commands::NewWallet => {
            let wallet = Wallet::generate(cfg.network);
            wallet.save(&cfg.wallet_path)?;
        }
        Commands::Balance => {
            let wallet = cfg.wallet()?;
            let addr = wallet.address(cfg.network)?;
            let balance = cfg.rpc_client()?.get_balance(&addr)?;
            println!("{}", balance);
        }
        Commands::Watch => {
            let wallet = cfg.wallet()?;
            let addr = wallet.address(cfg.network)?;
            cfg.rpc_client()?.watch_address(&[&addr])?;
        }
        Commands::DecodeBase58 { src } => {
            let bytes = decode_base58(&src)?;
            println!("{:?}", as_hex(&bytes));
        }
        Commands::Prepare { receiver, output } => {
            let raw: Vec<(&str, u64)> = receiver
                .iter()
                .map(|r| parse_receiver(r))
                .collect::<Result<_>>()?;
            let receivers = Receivers::parse(&raw, cfg.network)?;
            let tm = TransactionManager::new(cfg.wallet()?, cfg.network);
            let params = tm.prepare(cfg.rpc_client()?, receivers)?;
            params.save(&output)?;
            eprintln!("Params written to {}", output);
        }
        Commands::Sign { params } => {
            let tx_params = TransactionParams::load(&params, cfg.network)?;
            let tm = TransactionManager::new(cfg.wallet()?, cfg.network);
            println!("{}", tm.sign(&tx_params)?);
        }
    }

    Ok(())
}

/// Parse "address:amount_sat" into (address, amount_sat).
fn parse_receiver(s: &str) -> Result<(&str, u64)> {
    let (addr, amt) = s
        .rsplit_once(':')
        .context("receiver must be in 'address:amount_sat' format")?;
    let sat: u64 = amt.parse().context("invalid amount_sat")?;
    Ok((addr, sat))
}

#[cfg(test)]
mod tests {}
