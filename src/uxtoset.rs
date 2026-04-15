use crate::{input_select_strategy::UtxoInputSelectStrategy, utils::Satoshi};
use anyhow::Result;
use bitcoin::{Amount, ScriptBuf, Txid};
use bitcoincore_rpc::json::ListUnspentResultEntry;

// Custom Utxo type to decouple the dependency to rpc client implementations
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Utxo {
    pub txid: Txid,
    pub vout: u32,
    pub amount: Amount,
    pub script_pub_key: ScriptBuf,
}

impl From<&ListUnspentResultEntry> for Utxo {
    fn from(value: &ListUnspentResultEntry) -> Self {
        Self {
            txid: value.txid,
            vout: value.vout,
            amount: value.amount,
            script_pub_key: value.script_pub_key.clone(),
        }
    }
}
pub struct UtxoSet<T: UtxoInputSelectStrategy> {
    utxos: Vec<Utxo>,
    select_strategy: T,
}

impl<T: UtxoInputSelectStrategy> UtxoSet<T> {
    pub fn new(utxos: Vec<Utxo>, select_strategy: T) -> Self {
        Self {
            utxos,
            select_strategy,
        }
    }

    pub fn select_input(
        &self,
        amount: Amount,
        output_count: u64,
        fee_rate: Amount,
    ) -> Result<Vec<Utxo>> {
        T::select_input(&self.utxos, amount, output_count, fee_rate)
    }

    pub fn balance(&self) -> Satoshi {
        self.utxos.iter().map(|e| e.amount.to_sat()).sum()
    }
}
