pub mod min;

use crate::uxtoset::Utxo;
use anyhow::Result;
use bitcoin::Amount;

pub trait UtxoInputSelectStrategy {
    /// Select the set of UTXOs that reach the amount, excluding the UTXOs that cost more fee that its value.
    fn select_input(
        &self,
        utxos: &[Utxo],
        amount: Amount,
        output_count: u64,
        fee_rate: Amount,
    ) -> Result<Vec<Utxo>>;
}
