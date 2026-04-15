pub mod localrpc;

use crate::utxoset::UtxoSet;
use anyhow::Result;
use bitcoin::{Address, Amount};

pub trait BtcClient {
    /// Get all unspent transactions for address addr
    fn get_utxo_set(&self, addr: &Address) -> Result<UtxoSet>;
    /// Get the sum of balance of all UTXOs for addr
    fn get_balance(&self, addr: &Address) -> Result<Amount>;
    /// Get fee rate
    fn get_fee_rate(&self) -> Result<Amount>;
}
