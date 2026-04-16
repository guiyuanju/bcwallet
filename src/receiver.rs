use anyhow::{bail, Context, Result};
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

/// Unchecked collection of receivers, deserialized directly from JSON.
/// Call `.check(network)` to validate all addresses at once.
#[derive(Serialize, Deserialize)]
pub struct ReceiversUnchecked(pub Vec<ReceiverUnchecked>);

impl ReceiversUnchecked {
    pub fn check(self, network: Network) -> Result<Receivers> {
        if self.0.is_empty() {
            bail!("at least one receiver is required");
        }
        let items: Vec<Receiver> = self
            .0
            .into_iter()
            .map(|r| r.check(network))
            .collect::<Result<_>>()?;
        Ok(Receivers(items))
    }
}

/// A collection of validated receivers for a transaction.
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
            items.push(Receiver::new(addr, Amount::from_sat(sat)));
        }
        Ok(Self(items))
    }

    pub fn total_out(&self) -> Amount {
        self.0.iter().map(|r| r.amount).sum()
    }

    pub fn push(&mut self, receiver: Receiver) {
        self.0.push(receiver);
    }

    pub fn receivers(&self) -> &[Receiver] {
        &self.0
    }

    /// Sum the serialized vbytes of all outputs (8 bytes value + 1 byte script len + script).
    pub fn output_vbytes(&self) -> u64 {
        let mut total = 0u64;
        for r in &self.0 {
            let script_len = r.address.script_pubkey().len() as u64;
            total += 8 + 1 + script_len;
        }
        total
    }
}

impl From<Receivers> for Vec<ReceiverUnchecked> {
    fn from(rs: Receivers) -> Self {
        rs.0.iter().map(ReceiverUnchecked::from).collect()
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
        let result = Receivers::parse(&[], Network::Testnet);
        assert!(result.is_err());
    }
}
