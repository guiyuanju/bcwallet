use crate::input_select_strategy::UtxoInputSelectStrategy;
use anyhow::Result;
use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, TxIn, Txid, Witness};
use bitcoincore_rpc::json::ListUnspentResultEntry;

// Custom Utxo type to decouple the dependency to rpc client implementations
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Utxo {
    pub txid: Txid,
    pub vout: u32,
    pub amount: Amount,
    pub script_pubkey: ScriptBuf,
}

impl From<&ListUnspentResultEntry> for Utxo {
    fn from(value: &ListUnspentResultEntry) -> Self {
        Self {
            txid: value.txid,
            vout: value.vout,
            amount: value.amount,
            script_pubkey: value.script_pub_key.clone(),
        }
    }
}

impl From<&Utxo> for TxIn {
    fn from(value: &Utxo) -> Self {
        TxIn {
            previous_output: OutPoint::new(value.txid, value.vout),
            script_sig: ScriptBuf::default(),
            sequence: Sequence::MAX,
            witness: Witness::default(),
        }
    }
}

impl From<Utxo> for TxIn {
    fn from(value: Utxo) -> Self {
        (&value).into()
    }
}

pub struct UtxoSet {
    utxos: Vec<Utxo>,
}

impl UtxoSet {
    pub fn new(utxos: Vec<Utxo>) -> Self {
        Self { utxos }
    }

    pub fn select_input<T>(
        &self,
        amount: Amount,
        output_vbytes: u64,
        fee_rate: Amount,
        strategy: T,
    ) -> Result<(UtxoSet, Amount)>
    where
        T: UtxoInputSelectStrategy,
    {
        strategy.select_input(&self.utxos, amount, output_vbytes, fee_rate)
    }

    pub fn balance(&self) -> Amount {
        Amount::from_sat(self.utxos.iter().map(|e| e.amount.to_sat()).sum())
    }

    pub fn utxos(&self) -> &[Utxo] {
        &self.utxos
    }
}
