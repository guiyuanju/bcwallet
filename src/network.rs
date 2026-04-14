use crate::types::Satoshi;
use anyhow::Result;
use bitcoin::Address;
use bitcoincore_rpc::json::ListUnspentResultEntry;

pub trait NetworkClient {
    /// Get all unspent transactions for address addr.
    fn get_uxtos(&self, addr: &Address) -> Result<Vec<ListUnspentResultEntry>>;
    /// Get the sum of balance of all UTXOs for addr.
    fn get_balance(&self, addr: &Address) -> Result<Satoshi>;
}
