use anyhow::{bail, Context, Result};
use bitcoin::{Address, Network, PrivateKey, PublicKey};
use secp256k1::{rand::rngs::OsRng, Secp256k1, SecretKey};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Write},
    str::FromStr,
};

/// JSON representation of wallet on disk.
#[derive(Serialize, Deserialize)]
pub struct WalletUnchecked {
    pub private_key: String,
    pub address: String,
}

/// In-memory wallet with pre-parsed key material.
pub struct Wallet {
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
    pub address: Address,
    pub network: Network,
}

impl Wallet {
    pub fn new(pk: &str, addr: &str, network: Network) -> Result<Self> {
        Wallet::parse(
            WalletUnchecked {
                private_key: pk.to_owned(),
                address: addr.to_owned(),
            },
            network,
        )
    }

    pub fn parse(w: WalletUnchecked, network: Network) -> Result<Self> {
        let privkey =
            PrivateKey::from_wif(&w.private_key).context("failed to parse private key WIF")?;
        let secp = Secp256k1::new();
        let public_key = PublicKey::from_private_key(&secp, &privkey);
        let expected_addr = Address::p2pkh(public_key, network);

        if !w.address.is_empty() {
            let stored_addr = Address::from_str(&w.address)?
                .require_network(network)
                .context("wallet was created for a different network")?;
            if stored_addr != expected_addr {
                bail!("wallet address does not match private key");
            }
        }

        Ok(Wallet {
            secret_key: privkey.inner,
            public_key,
            address: expected_addr,
            network,
        })
    }

    pub fn generate(network: Network) -> Self {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut OsRng);
        let public_key = PublicKey::new(public_key);
        let address = Address::p2pkh(public_key, network);

        Self {
            secret_key,
            public_key,
            address,
            network,
        }
    }

    pub fn from_file(file_path: &str, network: Network) -> Result<Self> {
        let file = File::open(file_path).context("failed to open wallet file")?;
        let reader = BufReader::new(file);
        let w: WalletUnchecked =
            serde_json::from_reader(reader).context("failed to deserialize wallet file")?;
        Wallet::parse(w, network)
    }

    pub fn save(&self, file_path: &str) -> Result<()> {
        let wf = WalletUnchecked {
            private_key: PrivateKey::new(self.secret_key, self.network).to_wif(),
            address: self.address.to_string(),
        };
        let json =
            serde_json::to_string_pretty(&wf).context("failed to serialize wallet to json")?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(file_path)?;
        writeln!(file, "{}", json).context("failed to write to wallet")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_match_addr() {
        let wallet = Wallet::generate(Network::Testnet);

        let secp = Secp256k1::new();
        let privkey = PrivateKey::new(wallet.secret_key, wallet.network);
        let addr = Address::p2pkh(
            PublicKey::from_private_key(&secp, &privkey),
            Network::Testnet,
        );

        assert_eq!(&addr, &wallet.address);
    }
}
