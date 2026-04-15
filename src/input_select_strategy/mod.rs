pub mod min;

use crate::utxoset::{Utxo, UtxoSet};
use anyhow::Result;
use bitcoin::Amount;

pub trait UtxoInputSelectStrategy {
    /// Select the set of UTXOs that reach the amount, excluding the UTXOs that cost more fee that its value,
    /// return (selected UTXO set, estimated fee).
    fn select_input(
        &self,
        utxos: &[Utxo],
        amount: Amount,
        output_vbytes: u64,
        fee_rate: Amount,
    ) -> Result<(UtxoSet, Amount)>;
}
