use crate::types::Satoshi;
use anyhow::Result;
use bitcoin::{Address, Amount, ScriptBuf, Txid};
use bitcoincore_rpc::json::ListUnspentResultEntry;
use std::cmp;

pub trait BtcClient {
    /// Get all unspent transactions for address addr.
    fn get_uxtos(&self, addr: &Address) -> Result<Vec<Utxo>>;
    /// Get the sum of balance of all UTXOs for addr.
    fn get_balance(&self, addr: &Address) -> Result<Satoshi>;
}

// Custom Utxo type to decouple the dependency to rpc client implementations
#[derive(Debug, PartialEq, Eq)]
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

impl Ord for Utxo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.amount.cmp(&other.amount)
    }
}

impl PartialOrd for Utxo {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub trait UtxoInputSelectStrategy {
    fn select_input(utxos: &[Utxo], amount: Amount, fee: Amount) -> Result<Vec<Utxo>>;
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

    fn select_input(&self, amount: Amount, fee: Amount) -> Result<Vec<Utxo>> {
        T::select_input(&self.utxos, amount, fee)
    }
}
