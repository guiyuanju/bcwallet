pub mod localrpc;

use crate::{utils::Satoshi, uxtoset::Utxo};
use anyhow::Result;
use bitcoin::Address;

pub trait BtcClient {
    /// Get all unspent transactions for address addr.
    fn get_uxtos(&self, addr: &Address) -> Result<Vec<Utxo>>;
    /// Get the sum of balance of all UTXOs for addr.
    fn get_balance(&self, addr: &Address) -> Result<Satoshi>;
}
