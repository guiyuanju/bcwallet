use crate::params::{Receiver, ReceiverUnchecked};
use crate::utxo::Utxo;
use crate::wallet::{Wallet, WalletFile};
use anyhow::{bail, Result};
use bitcoin::{Address, Amount, Network, Txid};
use std::str::FromStr;

pub const SENDER: &str = "mwqmgMkf6ZsX2wxSK6GA2JRMVswBo29UWX";
pub const RECEIVER: &str = "tb1qerzrlxcfu24davlur5sqmgzzgsal6wusda40er";
pub const DUMMY_TXID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

pub fn test_address() -> Address {
    Address::from_str(SENDER)
        .unwrap()
        .require_network(Network::Testnet)
        .unwrap()
}

pub fn test_utxo(sat: u64) -> Utxo {
    Utxo {
        txid: Txid::from_str(DUMMY_TXID).unwrap(),
        vout: 0,
        amount: Amount::from_sat(sat),
        script_pubkey: test_address().script_pubkey(),
    }
}

pub fn stub_wallet() -> Wallet {
    WalletFile {
        private_key: "cQ7YsHdL8Spm8qv7V6weuV7MskGcF6cfZk4AaNkE1aG8nVGGjTaM".to_string(),
        address: SENDER.to_string(),
    }
    .into_wallet(Network::Testnet)
    .unwrap()
}

/// Parse raw `(address_str, satoshi)` pairs into validated receivers.
pub fn parse_receivers(raw: &[(&str, u64)], network: Network) -> Result<Vec<Receiver>> {
    if raw.is_empty() {
        bail!("at least one receiver is required");
    }
    raw.iter()
        .map(|&(addr, sat)| {
            ReceiverUnchecked {
                address: addr.to_string(),
                amount_sat: sat,
            }
            .check(network)
        })
        .collect()
}
