use crate::utxoset::{Utxo, UtxoSet};
use anyhow::{Context, Result, bail};
use bitcoin::{
    Address, Amount, Network, ScriptBuf, Transaction, TxOut, Txid, absolute::LockTime,
    transaction::Version,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufReader, Write},
    str::FromStr,
};

/// A single output in the transaction.
#[derive(Serialize, Deserialize)]
pub struct Receiver {
    pub(crate) address: String,
    pub(crate) amount_sat: u64,
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
            let addr = Address::from_str(addr_str)?.require_network(network)?;
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

/// Parameters needed to construct and sign a transaction offline.
/// Written by `prepare` (online) and read by `sign` (offline).
#[derive(Serialize, Deserialize)]
pub struct TransactionParams {
    pub(crate) receivers: Vec<Receiver>,
    pub(crate) utxos: Vec<UtxoParam>,
}

#[derive(Serialize, Deserialize)]
pub struct UtxoParam {
    pub(crate) txid: String,
    pub(crate) vout: u32,
    pub(crate) amount_sat: u64,
    pub(crate) script_pubkey: String, // hex-encoded
}

impl UtxoParam {
    pub fn from_utxo(utxo: &Utxo) -> Self {
        Self {
            txid: utxo.txid.to_string(),
            vout: utxo.vout,
            amount_sat: utxo.amount.to_sat(),
            script_pubkey: utxo.script_pubkey.to_hex_string(),
        }
    }

    pub fn to_utxo(&self) -> Result<Utxo> {
        Ok(Utxo {
            txid: Txid::from_str(&self.txid).context("invalid txid")?,
            vout: self.vout,
            amount: Amount::from_sat(self.amount_sat),
            script_pubkey: ScriptBuf::from_hex(&self.script_pubkey)
                .context("invalid script_pubkey hex")?,
        })
    }
}

impl TransactionParams {
    pub fn new(receivers: Vec<Receiver>, utxo_set: &UtxoSet) -> Self {
        Self {
            receivers,
            utxos: utxo_set.utxos().iter().map(UtxoParam::from_utxo).collect(),
        }
    }

    pub fn to_utxo_set(&self) -> Result<UtxoSet> {
        let utxos: Vec<Utxo> = self
            .utxos
            .iter()
            .map(|u| u.to_utxo())
            .collect::<Result<_>>()?;
        Ok(UtxoSet::new(utxos))
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

    pub fn load(path: &str, network: Network) -> Result<Self> {
        let file = File::open(path).context("failed to open params file")?;
        let reader = BufReader::new(file);
        let params: Self =
            serde_json::from_reader(reader).context("failed to deserialize params file")?;
        params.validate(network)?;
        Ok(params)
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let json =
            serde_json::to_string_pretty(self).context("failed to serialize params to json")?;
        let mut file = File::create(path).context("failed to create params file")?;
        writeln!(file, "{}", json).context("failed to write params file")
    }

    fn validate(&self, network: Network) -> Result<()> {
        for r in &self.receivers {
            r.address(network)?;
        }
        for u in &self.utxos {
            u.to_utxo()?;
        }
        Ok(())
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
    fn test_receiver_roundtrip() {
        let addr = test_address();
        let r = Receiver::new(&addr, Amount::from_sat(1234));
        assert_eq!(r.amount_sat, 1234);
        assert_eq!(r.amount(), Amount::from_sat(1234));
        assert_eq!(r.address(Network::Testnet).unwrap(), addr);
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
    fn test_utxo_param_roundtrip() {
        let utxo = test_utxo();
        let param = UtxoParam::from_utxo(&utxo);
        let back = param.to_utxo().unwrap();
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

        let restored = params.to_utxo_set().unwrap();
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
        params.save(path).unwrap();

        let loaded = TransactionParams::load(path, Network::Testnet).unwrap();
        assert_eq!(loaded.receivers.len(), 1);
        assert_eq!(loaded.receivers[0].amount_sat, 3000);
        assert_eq!(loaded.utxos.len(), 1);
        assert_eq!(loaded.utxos[0].amount_sat, 50_000);

        std::fs::remove_file(path).ok();
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
    fn test_utxo_param_invalid_txid() {
        let p = UtxoParam {
            txid: "invalid".to_string(),
            vout: 0,
            amount_sat: 1000,
            script_pubkey: "76a914".to_string(),
        };
        assert!(p.to_utxo().is_err());
    }

    #[test]
    fn test_parse_empty_receivers_fails() {
        let result = Receivers::parse(&[], Network::Testnet);
        assert!(result.is_err());
    }
}
