#[cfg(test)]
use std::env;

use anyhow::{Context, Result};
use bitcoin::base58;

#[cfg(test)]
use crate::{btc_client::localrpc::LocalRpc, wallet::Wallet};

pub fn decode_base58(src: &str) -> Result<Vec<u8>> {
    base58::decode(src).context("failed to decode as base58")
}

pub fn as_hex(bytes: &[u8]) -> String {
    let res: Vec<String> = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    res.concat()
}

#[cfg(test)]
pub fn load_wallet() -> Wallet {
    let wallet_path = env::var("WALLET").unwrap_or("wallet.json".to_owned());
    let mut wallet = Wallet::new();
    wallet.load(&wallet_path).unwrap();
    wallet
}

#[cfg(test)]
pub fn new_rpc_client() -> LocalRpc {
    let port = env::var("BTC_RPC_PORT").unwrap_or("18332".to_owned());
    let username = env::var("BTC_RPC_USER").unwrap_or("user".to_owned());
    let passwd = env::var("BTC_RPC_PASS").unwrap_or("passwd".to_owned());
    let client = LocalRpc::new(&port, &username, &passwd).unwrap();
    client
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_wif_to_hex() {
        let testnet_compressed_private_key_wif_based58 =
            "cQ7YsHdL8Spm8qv7V6weuV7MskGcF6cfZk4AaNkE1aG8nVGGjTaM";
        // ef - verison number, testnet
        // 01 - compression flag, public key should use compressed format
        // ada6f896 - checksum(version, key, flag)
        let hex = "ef4b8b6361b84e44097a3a2e46702af586c8a081c420277aab5b0d4cf897faca0801ada6f896";
        assert_eq!(
            hex,
            as_hex(&decode_base58(testnet_compressed_private_key_wif_based58).unwrap())
        );
    }
}
