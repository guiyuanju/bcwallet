//! Defines the data transfer format for encoding and decoding e.g. `params.json`.
//! The online process produce these types of data, persisted to disk,
//! and read by offline process part and sign.

use crate::utxo::Utxo;
use anyhow::{Context, Result};
use bitcoin::{absolute, transaction::Version, Address, Amount, Network, Transaction, TxOut};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufReader, Read, Write},
    str::FromStr,
};

/// Unchecked receiver parsed from JSON or CLI input
#[derive(Serialize, Deserialize, Clone)]
pub struct ReceiverUnchecked {
    pub address: String,
    pub amount_sat: u64,
}

impl ReceiverUnchecked {
    /// Validate address for the given network
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

// For clap to parse command line argument
impl FromStr for ReceiverUnchecked {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let (addr, amt) = s
            .rsplit_once(':')
            .context("receiver must be in 'address:amount_sat' format")?;
        let amount_sat: u64 = amt.parse().context("invalid amount_sat")?;
        Ok(Self {
            address: addr.to_string(),
            amount_sat,
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

/// A validated receiver with a network-checked address and amount
pub struct Receiver {
    pub address: Address,
    pub amount: Amount,
}

impl Receiver {
    pub fn new(address: Address, amount: Amount) -> Self {
        Self { address, amount }
    }

    pub fn from_raw(address: &str, amount: u64, network: Network) -> Result<Self> {
        Ok(Self {
            address: Address::from_str(address)
                .context("failed to convert str to address")?
                .require_network(network)?,
            amount: Amount::from_sat(amount),
        })
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

/// Sum the serialized vbytes of all outputs (8 bytes value + 1 byte script len + script)
pub fn output_vbytes(receivers: &[Receiver]) -> u64 {
    receivers
        .iter()
        .map(|r| 8 + 1 + r.address.script_pubkey().len() as u64)
        .sum()
}

/// Unchecked transaction parameters parsed from JSON
#[derive(Serialize, Deserialize)]
pub struct TransactionParamUnchecked {
    pub receivers: Vec<ReceiverUnchecked>,
    pub utxos: Vec<Utxo>,
}

impl TransactionParamUnchecked {
    pub fn new(receivers: Vec<ReceiverUnchecked>, uxtos: &[Utxo]) -> Self {
        Self {
            receivers,
            utxos: uxtos.to_vec(),
        }
    }

    pub fn check(self, network: Network) -> Result<TransactionParam> {
        let receivers: Vec<Receiver> = self
            .receivers
            .into_iter()
            .map(|r| r.check(network))
            .collect::<Result<_>>()?;
        Ok(TransactionParam {
            receivers,
            utxos: self.utxos,
        })
    }

    pub fn from_reader(reader: impl Read) -> Result<Self> {
        serde_json::from_reader(reader).context("failed to deserialize params")
    }

    pub fn write_to(&self, mut writer: impl Write) -> Result<()> {
        let json =
            serde_json::to_string_pretty(self).context("failed to serialize params to json")?;
        writeln!(writer, "{}", json).context("failed to write params")
    }

    pub fn from_file(path: &str) -> Result<Self> {
        let file = File::open(path).context("failed to open params file")?;
        Self::from_reader(BufReader::new(file))
    }

    pub fn save_as_file(&self, path: &str) -> Result<()> {
        let file = File::create(path).context("failed to create params file")?;
        self.write_to(file)
    }
}

/// Validated transaction parameters with checked addresses and amounts
pub struct TransactionParam {
    pub receivers: Vec<Receiver>,
    pub utxos: Vec<Utxo>,
}

impl TransactionParam {
    pub fn new(receivers: Vec<Receiver>, utxos: Vec<Utxo>) -> Self {
        Self { receivers, utxos }
    }

    pub fn write_to(&self, writer: impl Write) -> Result<()> {
        let unchecked = TransactionParamUnchecked::new(
            self.receivers.iter().map(ReceiverUnchecked::from).collect(),
            &self.utxos,
        );
        unchecked.write_to(writer)
    }

    pub fn save_as_file(&self, path: &str) -> Result<()> {
        let file = File::create(path).context("failed to create params file")?;
        self.write_to(file)
    }
}

impl From<&TransactionParam> for Transaction {
    fn from(value: &TransactionParam) -> Self {
        let outputs: Vec<TxOut> = value.receivers.iter().map(TxOut::from).collect();
        Transaction {
            version: Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: value.utxos.iter().map(|u| u.into()).collect(),
            output: outputs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{Amount, Network, Txid};

    const SENDER: &str = "mwqmgMkf6ZsX2wxSK6GA2JRMVswBo29UWX";

    fn test_address() -> Address {
        Address::from_str(SENDER)
            .unwrap()
            .require_network(Network::Testnet)
            .unwrap()
    }

    fn test_utxo(sat: u64) -> Utxo {
        Utxo {
            txid: Txid::from_str(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .unwrap(),
            vout: 0,
            amount: Amount::from_sat(sat),
            script_pubkey: test_address().script_pubkey(),
        }
    }

    fn unchecked_receiver(addr: &Address, sat: u64) -> ReceiverUnchecked {
        ReceiverUnchecked::from(&Receiver::new(addr.clone(), Amount::from_sat(sat)))
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
    fn test_receiver_unchecked_from_str() {
        let r: ReceiverUnchecked = "tb1qaddr:5000".parse().unwrap();
        assert_eq!(r.address, "tb1qaddr");
        assert_eq!(r.amount_sat, 5000);
    }

    #[test]
    fn test_receiver_unchecked_from_str_bad_format() {
        assert!("no_colon_here".parse::<ReceiverUnchecked>().is_err());
        assert!("addr:notanumber".parse::<ReceiverUnchecked>().is_err());
    }

    #[test]
    fn test_transaction_params_check() {
        let addr = test_address();
        let unchecked = TransactionParamUnchecked::new(
            vec![unchecked_receiver(&addr, 10_000)],
            &[test_utxo(50_000)],
        );
        let params = unchecked.check(Network::Testnet).unwrap();

        assert_eq!(params.receivers.len(), 1);
        assert_eq!(params.receivers[0].amount, Amount::from_sat(10_000));
        assert_eq!(params.utxos.len(), 1);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let addr = test_address();
        let unchecked = TransactionParamUnchecked::new(
            vec![unchecked_receiver(&addr, 3000)],
            &[test_utxo(50_000)],
        );

        let mut buf = Vec::new();
        unchecked.write_to(&mut buf).unwrap();

        let loaded = TransactionParamUnchecked::from_reader(buf.as_slice()).unwrap();
        assert_eq!(loaded.receivers.len(), 1);
        assert_eq!(loaded.receivers[0].amount_sat, 3000);
        assert_eq!(loaded.utxos.len(), 1);
        assert_eq!(loaded.utxos[0].amount.to_sat(), 50_000);

        // Verify check works on loaded params
        let checked = loaded.check(Network::Testnet).unwrap();
        assert_eq!(checked.receivers.len(), 1);
        assert_eq!(checked.receivers[0].amount, Amount::from_sat(3000));
    }

    #[test]
    fn test_utxo_invalid_txid_deserialize() {
        let json = r#"{"txid":"invalid","vout":0,"amount_sat":1000,"script_pubkey":"76a914"}"#;
        let result: std::result::Result<Utxo, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
