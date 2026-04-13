use anyhow::{Context, Result};
use bitcoin::{Address, Network, PrivateKey, PublicKey};
use secp256k1::{Secp256k1, rand::rngs::OsRng};
use serde::Serialize;
use std::{fs, str::FromStr};

#[derive(Serialize)]
pub struct Wallet {
    private_key: String,
    address: String,
}

impl Wallet {
    pub fn new() -> Self {
        Self {
            private_key: "".to_owned(),
            address: "".to_owned(),
        }
    }

    pub fn compute_key_addr(&mut self) {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut OsRng);
        let private_key = PrivateKey::new(secret_key, Network::Testnet);
        let address = Address::p2pkh(PublicKey::new(public_key), Network::Testnet);

        self.private_key = private_key.to_wif();
        self.address = address.to_string();
    }

    pub fn private_key(&self) -> Result<PrivateKey> {
        PrivateKey::from_wif(&self.private_key).context("failed to convert string to private key")
    }

    pub fn address(&self) -> Result<Address> {
        let addr = Address::from_str(&self.address)?;
        addr.require_network(Network::Testnet)
            .context("expect address to use testnet")
    }

    pub fn save(&self) -> Result<()> {
        let json =
            serde_json::to_string_pretty(self).context("falied to serialize wallet to json")?;
        fs::write("wallet.json", &json).context("failed to write to wallet")?;
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
