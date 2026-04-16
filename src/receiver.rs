use anyhow::{Context, Result};
use bitcoin::{Address, Amount, Network, TxOut};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Unchecked receiver parsed from JSON or CLI input
#[derive(Serialize, Deserialize)]
pub struct ReceiverUnchecked {
    pub address: String,
    pub amount_sat: u64,
}

impl ReceiverUnchecked {
    /// Check the network, prevent fund sent to the wrong network
    pub fn check(self, network: Network) -> Result<Receiver> {
        let address = Address::from_str(&self.address)?
            .require_network(network)
            .context("invalid receiver address")?;
        Ok(Receiver {
            address,
            amount: Amount::from_sat(self.amount_sat),
        })
    }
}

impl From<&Receiver> for ReceiverUnchecked {
    fn from(r: &Receiver) -> Self {
        Self {
            address: r.address.to_string(),
            amount_sat: r.amount.to_sat(),
        }
    }
}

/// A validated receiver with a checked address and amount
pub struct Receiver {
    pub address: Address,
    pub amount: Amount,
}

impl Receiver {
    pub fn new(address: Address, amount: Amount) -> Self {
        Self { address, amount }
    }
}

impl From<&Receiver> for TxOut {
    fn from(r: &Receiver) -> Self {
        TxOut {
            value: r.amount,
            script_pubkey: r.address.script_pubkey(),
        }
    }
}

/// Sum the serialized vbytes of all outputs (8 bytes value + 1 byte script len + script).
pub fn output_vbytes(receivers: &[Receiver]) -> u64 {
    receivers
        .iter()
        .map(|r| 8 + 1 + r.address.script_pubkey().len() as u64)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{parse_receivers, test_address};

    #[test]
    fn test_receiver_to_tx_out() {
        let addr = test_address();
        let r = Receiver::new(addr.clone(), Amount::from_sat(5000));
        let out = TxOut::from(&r);
        assert_eq!(out.value, Amount::from_sat(5000));
        assert_eq!(out.script_pubkey, addr.script_pubkey());
    }

    #[test]
    fn test_receiver_unchecked_invalid_address() {
        let r = ReceiverUnchecked {
            address: "not_a_valid_address".to_string(),
            amount_sat: 100,
        };
        assert!(r.check(Network::Testnet).is_err());
    }

    #[test]
    fn test_parse_empty_receivers_fails() {
        let result = parse_receivers(&[], Network::Testnet);
        assert!(result.is_err());
    }
}
