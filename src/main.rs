mod btcclient;
mod params;
mod transaction;
mod utxo;
mod wallet;

use crate::{
    btcclient::{BtcClient, RpcClient},
    params::{ReceiverUnchecked, TransactionParamUnchecked},
    transaction::TransactionManager,
    utxo::SmallestFirst,
    wallet::Wallet,
};
use anyhow::{bail, Result};
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
    /// Prepare transaction params file (online, requires RPC)
    Prepare {
        /// Receivers in "address:amount_sat" format (repeatable)
        #[arg(long, required = true, value_parser = clap::value_parser!(ReceiverUnchecked))]
        receiver: Vec<ReceiverUnchecked>,
        /// Output params file path
        #[arg(long, default_value = "params.json")]
        output: String,
    },
    /// Sign a transaction from params file (offline, no network)
    Sign {
        /// Path to params.json
        params: String,
    },
    /// Send a signed transaction hex to the network
    Send {
        /// The raw transaction hex to broadcast
        hex: String,
    },
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
            let balance = cfg.btc_client()?.get_balance(&wallet.address)?;
            println!("{}", balance);
        }
        Commands::Watch => {
            let wallet = cfg.wallet()?;
            cfg.btc_client()?.watch_addresses(&[&wallet.address])?;
        }
        Commands::Prepare { receiver, output } => {
            let receivers = receiver
                .into_iter()
                .map(|r| r.check(cfg.network))
                .collect::<Result<Vec<_>>>()?;

            // Generate and save unsigned transaction
            let tm = TransactionManager::new(cfg.wallet()?);
            let params = tm.prepare(&cfg.btc_client()?, receivers, &SmallestFirst)?;
            params.save_as_file(&output)?;

            println!("Params written to {}", output);
        }
        Commands::Sign { params } => {
            let tx_params = TransactionParamUnchecked::from_file(&params)?.check(cfg.network)?;
            let tm = TransactionManager::new(cfg.wallet()?);
            println!("{}", tm.sign(&tx_params)?);
        }
        Commands::Send { hex } => {
            let txid = cfg.btc_client()?.send_raw_transaction(&hex)?;
            println!("{txid}");
        }
    }

    Ok(())
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

    fn btc_client(&self) -> Result<RpcClient> {
        RpcClient::new(&self.rpc_port, &self.rpc_user, &self.rpc_pass)
    }

    fn wallet(&self) -> Result<Wallet> {
        Wallet::from_file(&self.wallet_path, self.network)
    }
}
