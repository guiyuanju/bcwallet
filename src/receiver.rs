use anyhow::{bail, Context, Result};
use bitcoin::{Address, Amount, Network, TxOut};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// A single output in the transaction.
#[derive(Serialize, Deserialize)]
pub struct Receiver {
    pub address: String,
    pub amount_sat: u64,
}

impl Receiver {
    pub fn new(address: &Address, amount: Amount) -> Self {
        Self {
            address: address.to_string(),
            amount_sat: amount.to_sat(),
        }
    }

    pub fn address(&self, network: Network) -> Result<Address> {
        Address::from_str(&self.address)?
            .require_network(network)
            .context("invalid receiver address")
    }

    pub fn amount(&self) -> Amount {
        Amount::from_sat(self.amount_sat)
    }

    pub fn to_tx_out(&self, network: Network) -> Result<TxOut> {
        Ok(TxOut {
            value: self.amount(),
            script_pubkey: self.address(network)?.script_pubkey(),
        })
    }
}

/// A collection of receivers for a transaction.
pub struct Receivers(Vec<Receiver>);

impl Receivers {
    /// Parse raw `(address_str, satoshi)` pairs into validated receivers.
    pub fn parse(raw: &[(&str, u64)], network: Network) -> Result<Self> {
        if raw.is_empty() {
            bail!("at least one receiver is required");
        }
        let mut items = Vec::with_capacity(raw.len());
        for &(addr_str, sat) in raw {
            let addr = Address::from_str(addr_str)
                .with_context(|| format!("invalid address: {addr_str}"))?
                .require_network(network)?;
            items.push(Receiver::new(&addr, Amount::from_sat(sat)));
        }
        Ok(Self(items))
    }

    pub fn total_out(&self) -> Amount {
        self.0.iter().map(|r| r.amount()).sum()
    }

    pub fn push(&mut self, receiver: Receiver) {
        self.0.push(receiver);
    }

    pub fn into_inner(self) -> Vec<Receiver> {
        self.0
    }

    /// Sum the serialized vbytes of all outputs (8 bytes value + 1 byte script len + script).
    pub fn output_vbytes(&self, network: Network) -> Result<u64> {
        let mut total = 0u64;
        for r in &self.0 {
            let script_len = r.address(network)?.script_pubkey().len() as u64;
            total += 8 + 1 + script_len;
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address() -> Address {
        Address::from_str("mwqmgMkf6ZsX2wxSK6GA2JRMVswBo29UWX")
            .unwrap()
            .require_network(Network::Testnet)
            .unwrap()
    }

    #[test]
    fn test_receiver_to_tx_out() {
        let addr = test_address();
        let r = Receiver::new(&addr, Amount::from_sat(5000));
        let out = r.to_tx_out(Network::Testnet).unwrap();
        assert_eq!(out.value, Amount::from_sat(5000));
        assert_eq!(out.script_pubkey, addr.script_pubkey());
    }

    #[test]
    fn test_receiver_invalid_address() {
        let r = Receiver {
            address: "not_a_valid_address".to_string(),
            amount_sat: 100,
        };
        assert!(r.address(Network::Testnet).is_err());
        assert!(r.to_tx_out(Network::Testnet).is_err());
    }

    #[test]
    fn test_parse_empty_receivers_fails() {
        let result = Receivers::parse(&[], Network::Testnet);
        assert!(result.is_err());
    }
}
