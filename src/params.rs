use crate::receiver::Receiver;
use crate::utxoset::{Utxo, UtxoSet};
use anyhow::{Context, Result};
use bitcoin::{absolute::LockTime, transaction::Version, Network, Transaction, TxOut};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufReader, Write},
};

/// Parameters needed to construct and sign a transaction offline.
/// Written by `prepare` (online) and read by `sign` (offline).
#[derive(Serialize, Deserialize)]
pub struct TransactionParams {
    pub receivers: Vec<Receiver>,
    pub utxos: Vec<Utxo>,
}

impl TransactionParams {
    pub fn new(receivers: Vec<Receiver>, utxo_set: &UtxoSet) -> Self {
        Self {
            receivers,
            utxos: utxo_set.utxos().to_vec(),
        }
    }

    pub fn to_utxo_set(&self) -> UtxoSet {
        UtxoSet::new(self.utxos.clone())
    }

    /// Construct an unsigned transaction from these params
    pub fn to_unsigned_tx(&self, utxo_set: &UtxoSet, network: Network) -> Result<Transaction> {
        let outputs: Vec<TxOut> = self
            .receivers
            .iter()
            .map(|r| r.to_tx_out(network))
            .collect::<Result<_>>()?;

        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: utxo_set.utxos().iter().map(|u| u.into()).collect(),
            output: outputs,
        };

        Ok(tx)
    }

    pub fn from_file(path: &str, network: Network) -> Result<Self> {
        let file = File::open(path).context("failed to open params file")?;
        let reader = BufReader::new(file);
        let params: Self =
            serde_json::from_reader(reader).context("failed to deserialize params file")?;
        params.validate(network)?;
        Ok(params)
    }

    pub fn save_as_file(&self, path: &str) -> Result<()> {
        let json =
            serde_json::to_string_pretty(self).context("failed to serialize params to json")?;
        let mut file = File::create(path).context("failed to create params file")?;
        writeln!(file, "{}", json).context("failed to write params file")
    }

    fn validate(&self, network: Network) -> Result<()> {
        for r in &self.receivers {
            r.address(network)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utxoset::{Utxo, UtxoSet};
    use bitcoin::{Address, Amount, Network, Txid};
    use std::str::FromStr;
    fn test_address() -> Address {
        Address::from_str("mwqmgMkf6ZsX2wxSK6GA2JRMVswBo29UWX")
            .unwrap()
            .require_network(Network::Testnet)
            .unwrap()
    }

    fn test_utxo() -> Utxo {
        Utxo {
            txid: Txid::from_str(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .unwrap(),
            vout: 0,
            amount: Amount::from_sat(50_000),
            script_pubkey: test_address().script_pubkey(),
        }
    }

    #[test]
    fn test_utxo_serde_roundtrip() {
        let utxo = test_utxo();
        let json = serde_json::to_string(&utxo).unwrap();
        let back: Utxo = serde_json::from_str(&json).unwrap();
        assert_eq!(back, utxo);
    }

    #[test]
    fn test_transaction_params_to_unsigned_tx() {
        let addr = test_address();
        let receivers = vec![Receiver::new(&addr, Amount::from_sat(10_000))];
        let utxo_set = UtxoSet::new(vec![test_utxo()]);
        let params = TransactionParams::new(receivers, &utxo_set);

        let tx = params.to_unsigned_tx(&utxo_set, Network::Testnet).unwrap();
        assert_eq!(tx.input.len(), 1);
        assert_eq!(tx.output.len(), 1);
        assert_eq!(tx.output[0].value, Amount::from_sat(10_000));
    }

    #[test]
    fn test_transaction_params_to_utxo_set() {
        let utxo_set = UtxoSet::new(vec![test_utxo()]);
        let params = TransactionParams::new(vec![], &utxo_set);

        let restored = params.to_utxo_set();
        assert_eq!(restored.utxos().len(), 1);
        assert_eq!(restored.balance(), Amount::from_sat(50_000));
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let addr = test_address();
        let receivers = vec![Receiver::new(&addr, Amount::from_sat(3000))];
        let utxo_set = UtxoSet::new(vec![test_utxo()]);
        let params = TransactionParams::new(receivers, &utxo_set);

        let path = "/tmp/bcwallet_test_params.json";
        params.save_as_file(path).unwrap();

        let loaded = TransactionParams::from_file(path, Network::Testnet).unwrap();
        assert_eq!(loaded.receivers.len(), 1);
        assert_eq!(loaded.receivers[0].amount_sat, 3000);
        assert_eq!(loaded.utxos.len(), 1);
        assert_eq!(loaded.utxos[0].amount.to_sat(), 50_000);

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn test_utxo_invalid_txid_deserialize() {
        let json = r#"{"txid":"invalid","vout":0,"amount_sat":1000,"script_pubkey":"76a914"}"#;
        let result: std::result::Result<Utxo, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
