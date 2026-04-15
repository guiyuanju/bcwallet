use anyhow::{Context, Result, bail};
use bitcoin::{Address, Network, PrivateKey, PublicKey};
use secp256k1::{Secp256k1, SecretKey, rand::rngs::OsRng};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Write},
};

/// JSON representation of wallet on disk.
#[derive(Serialize, Deserialize)]
struct WalletFile {
    private_key: String,
    address: String,
}

/// In-memory wallet with pre-parsed key material.
pub struct Wallet {
    wif: String,
    secret_key: SecretKey,
    public_key: PublicKey,
    address: Address,
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
        }
    }

    pub fn from_file(file_path: &str, network: Network) -> Result<Self> {
        let file = File::open(file_path).context("failed to open wallet file")?;
        let reader = BufReader::new(file);
        let wf: WalletFile =
            serde_json::from_reader(reader).context("failed to deserialize wallet file")?;

        let privkey =
            PrivateKey::from_wif(&wf.private_key).context("failed to parse private key WIF")?;
        let secp = Secp256k1::new();
        let public_key = PublicKey::from_private_key(&secp, &privkey);
        let expected_addr = Address::p2pkh(public_key, network);

        if expected_addr.to_string() != wf.address {
            bail!(
                "wallet address does not match private key for network {}",
                network
            );
        }

        Ok(Self {
            wif: wf.private_key,
            secret_key: privkey.inner,
            public_key,
            address: expected_addr,
        })
    }

    pub fn save(&self, file_path: &str) -> Result<()> {
        let wf = WalletFile {
            private_key: self.wif.clone(),
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

    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }

    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    pub fn address(&self) -> &Address {
        &self.address
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

        assert_eq!(&addr, wallet.address());
    }
}
