use anyhow::{Context, Result, bail};
use bitcoin::{Address, Network, PrivateKey, PublicKey};
use secp256k1::{Secp256k1, SecretKey, rand::rngs::OsRng};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Write},
    str::FromStr,
};

#[derive(Serialize, Deserialize)]
pub struct Wallet {
    private_key: String, // Base58 encoded WIF secret key
    address: String,     // Base58 encoded compressed P2PKH
}

impl Wallet {
    /// Generate a new wallet with a fresh keypair.
    pub fn generate(network: Network) -> Self {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut OsRng);
        let private_key = PrivateKey::new(secret_key, network);
        let address = Address::p2pkh(PublicKey::new(public_key), network);

        Self {
            private_key: private_key.to_wif(),
            address: address.to_string(),
        }
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
    pub fn address(&self, network: Network) -> Result<Address> {
        let addr = Address::from_str(&self.address)?;
        addr.require_network(network)
            .context("address network mismatch")
    }

    /// Save current wallet to file, fail if already exists.
    pub fn save(&self, file_path: &str) -> Result<()> {
        let json =
            serde_json::to_string_pretty(self).context("failed to serialize wallet to json")?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(file_path)?;
        writeln!(file, "{}", json).context("failed to write to wallet")
    }

    /// Load wallet from file and validate that the address matches the private key.
    pub fn from_file(file_path: &str, network: Network) -> Result<Self> {
        let file = File::open(file_path).context("failed to open wallet file")?;
        let reader = BufReader::new(file);
        let wallet: Self =
            serde_json::from_reader(reader).context("failed to deserialize wallet file")?;
        wallet.validate(network)?;
        Ok(wallet)
    }

    fn validate(&self, network: Network) -> Result<()> {
        let secp = Secp256k1::new();
        let privkey = self.private_key()?;
        let pubkey = PublicKey::from_private_key(&secp, &privkey);
        let expected = Address::p2pkh(pubkey, network);
        if expected.to_string() != self.address {
            bail!(
                "wallet address does not match private key for network {}",
                network
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_match_addr() {
        let wallet = Wallet::generate(Network::Testnet);

        let scep = Secp256k1::new();
        let addr = Address::p2pkh(
            PublicKey::from_private_key(&scep, &wallet.private_key().unwrap()),
            Network::Testnet,
        );

        assert_eq!(addr, wallet.address(Network::Testnet).unwrap())
    }
}
