use anyhow::{Context, Result};
use bitcoin::{Address, Network, PrivateKey, PublicKey};
use secp256k1::{Secp256k1, SecretKey, rand::rngs::OsRng};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Write},
    mem::replace,
    str::FromStr,
};

#[derive(Serialize, Deserialize)]
pub struct Wallet {
    private_key: String, // Base58 encoded WIF secret key
    address: String,     // Base58 encoded compressed P2PKH
}

impl Wallet {
    /// Create an empty wallet.
    pub fn new() -> Self {
        Self {
            private_key: "".to_owned(),
            address: "".to_owned(),
        }
    }

    /// Compute and fill wallet with new private key and address.
    pub fn compute_key_addr(&mut self) {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut OsRng);
        let private_key = PrivateKey::new(secret_key, Network::Testnet);
        let address = Address::p2pkh(PublicKey::new(public_key), Network::Testnet);

        self.private_key = private_key.to_wif();
        self.address = address.to_string();
    }

    /// As bitcoin::PrivateKey.
    pub fn private_key(&self) -> Result<PrivateKey> {
        PrivateKey::from_wif(&self.private_key).context("failed to convert string to private key")
    }

    /// Get secret_key from private key.
    pub fn secret_key(&self) -> Result<SecretKey> {
        Ok(self.private_key()?.inner)
    }

    /// As bitcoin::PublicKey.
    pub fn public_key(&self) -> Result<PublicKey> {
        let secp = Secp256k1::new();
        let pubkey = secp256k1::PublicKey::from_secret_key(&secp, &self.private_key()?.inner);
        Ok(PublicKey::new(pubkey))
    }

    /// As bitcoin::Address.
    pub fn address(&self) -> Result<Address> {
        let addr = Address::from_str(&self.address)?;
        addr.require_network(Network::Testnet)
            .context("expect address to use testnet")
    }

    /// Save current wallet to file, fail if already exists.
    pub fn save(&self, file_path: &str) -> Result<()> {
        let json =
            serde_json::to_string_pretty(self).context("falied to serialize wallet to json")?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(file_path)?;
        writeln!(file, "{}", json).context("failed to write to wallet")
    }

    /// Load from wallet file, fail if not exists.
    pub fn load(&mut self, file_path: &str) -> Result<()> {
        let file = File::open(file_path).context("failed to open wallet file")?;
        let reader = BufReader::new(file);
        let wallet: Wallet =
            serde_json::from_reader(reader).context("failed to deserialize wallet file")?;
        _ = replace(self, wallet);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_match_addr() {
        let mut wallet = Wallet::new();
        wallet.compute_key_addr();

        let scep = Secp256k1::new();
        let addr = Address::p2pkh(
            PublicKey::from_private_key(&scep, &wallet.private_key().unwrap()),
            Network::Testnet,
        );

        assert_eq!(addr, wallet.address().unwrap())
    }
}
