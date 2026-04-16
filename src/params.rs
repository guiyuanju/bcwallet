use crate::receiver::{Receiver, ReceiverUnchecked};
use crate::utxoset::Utxo;
use anyhow::{Context, Result};
use bitcoin::Network;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufReader, Write},
};

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

    pub fn from_file(path: &str) -> Result<Self> {
        let file = File::open(path).context("failed to open params file")?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).context("failed to deserialize params file")
    }

    pub fn save_as_file(&self, path: &str) -> Result<()> {
        let json =
            serde_json::to_string_pretty(self).context("failed to serialize params to json")?;
        let mut file = File::create(path).context("failed to create params file")?;
        writeln!(file, "{}", json).context("failed to write params file")
    }
}

/// Validated transaction parameters with checked addresses and amounts
pub struct TransactionParam {
    pub receivers: Vec<Receiver>,
    pub utxos: Vec<Utxo>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utxoset::Utxo;
    use crate::valued::ValuedSlice;
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

    fn unchecked_receiver(addr: &Address, sat: u64) -> ReceiverUnchecked {
        ReceiverUnchecked::from(&crate::receiver::Receiver::new(
            addr.clone(),
            Amount::from_sat(sat),
        ))
    }

    #[test]
    fn test_transaction_params_to_unsigned_tx() {
        let addr = test_address();
        let unchecked =
            TransactionParamUnchecked::new(vec![unchecked_receiver(&addr, 10_000)], &[test_utxo()]);
        let params = unchecked.check(Network::Testnet).unwrap();

        assert_eq!(params.receivers.len(), 1);
        assert_eq!(params.receivers[0].amount, Amount::from_sat(10_000));
        assert_eq!(params.utxos.len(), 1);
    }

    #[test]
    fn test_transaction_params_to_utxo_set() {
        let utxos = vec![test_utxo()];
        let unchecked = TransactionParamUnchecked::new(vec![], &utxos);
        let params = unchecked.check(Network::Testnet).unwrap();

        assert_eq!(params.utxos.len(), 1);
        assert_eq!(utxos.total_value(), Amount::from_sat(50_000));
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let addr = test_address();
        let unchecked =
            TransactionParamUnchecked::new(vec![unchecked_receiver(&addr, 3000)], &[test_utxo()]);

        let path = "/tmp/bcwallet_test_params.json";
        unchecked.save_as_file(path).unwrap();

        let loaded = TransactionParamUnchecked::from_file(path).unwrap();
        assert_eq!(loaded.receivers.len(), 1);
        assert_eq!(loaded.receivers[0].amount_sat, 3000);
        assert_eq!(loaded.utxos.len(), 1);
        assert_eq!(loaded.utxos[0].amount.to_sat(), 50_000);

        // Verify check works on loaded params
        let checked = loaded.check(Network::Testnet).unwrap();
        assert_eq!(checked.receivers.len(), 1);
        assert_eq!(checked.receivers[0].amount, Amount::from_sat(3000));

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn test_utxo_invalid_txid_deserialize() {
        let json = r#"{"txid":"invalid","vout":0,"amount_sat":1000,"script_pubkey":"76a914"}"#;
        let result: std::result::Result<Utxo, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
