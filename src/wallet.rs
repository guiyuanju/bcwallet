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
pub struct WalletFile {
    pub private_key: String,
    pub address: String,
    #[serde(skip, default = "default_network")]
    pub network: Network,
}

fn default_network() -> Network {
    Network::Bitcoin
}

/// In-memory wallet with pre-parsed key material.
pub struct Wallet {
    pub wif: String,
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
    pub address: Address,
    pub network: Network,
}

impl Wallet {
    pub fn generate(network: Network) -> Self {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut OsRng);
        let public_key = PublicKey::new(public_key);
        let address = Address::p2pkh(public_key, network);
        let wif = PrivateKey::new(secret_key, network).to_wif();

        Self {
            wif,
            secret_key,
            public_key,
            address,
            network,
        }
    }

    pub fn from_file(file_path: &str, network: Network) -> Result<Self> {
        let file = File::open(file_path).context("failed to open wallet file")?;
        let reader = BufReader::new(file);
        let mut wf: WalletFile =
            serde_json::from_reader(reader).context("failed to deserialize wallet file")?;
        wf.network = network;
        Wallet::try_from(wf)
    }

    pub fn save(&self, file_path: &str) -> Result<()> {
        let wf = WalletFile {
            private_key: self.wif.clone(),
            address: self.address.to_string(),
            network: self.network,
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

impl TryFrom<WalletFile> for Wallet {
    type Error = anyhow::Error;

    fn try_from(wf: WalletFile) -> Result<Self> {
        let network = wf.network;
        let privkey =
            PrivateKey::from_wif(&wf.private_key).context("failed to parse private key WIF")?;
        let secp = Secp256k1::new();
        let public_key = PublicKey::from_private_key(&secp, &privkey);
        let expected_addr = Address::p2pkh(public_key, network);

        if !wf.address.is_empty() {
            let stored_addr = Address::from_str(&wf.address)?
                .require_network(network)
                .context("wallet was created for a different network")?;
            if stored_addr != expected_addr {
                bail!("wallet address does not match private key");
            }
        }

        Ok(Wallet {
            wif: wf.private_key,
            secret_key: privkey.inner,
            public_key,
            address: expected_addr,
            network,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_match_addr() {
        let wallet = Wallet::generate(Network::Testnet);

        let secp = Secp256k1::new();
        let addr = Address::p2pkh(
            PublicKey::from_private_key(&secp, &PrivateKey::from_wif(&wallet.wif).unwrap()),
            Network::Testnet,
        );

        assert_eq!(&addr, &wallet.address);
    }
}
