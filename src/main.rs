mod network;
mod restclient;
mod types;
mod wallet;

use crate::wallet::Wallet;
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new wallet file
    NewWallet,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::NewWallet => {
            let mut wallet = Wallet::new();
            wallet.compute_key_addr();
            wallet.save("wallet.json")?;
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
