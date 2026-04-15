use crate::{
    input_select_strategy::UtxoInputSelectStrategy,
    uxtoset::{Utxo, UtxoSet},
};
use anyhow::{Result, bail};
use bitcoin::Amount;

/// Input select strategy that selects small value first.
pub struct MinFirstStrategy();
impl UtxoInputSelectStrategy for MinFirstStrategy {
    fn select_input(
        &self,
        utxos: &[Utxo],
        amount: bitcoin::Amount,
        output_count: u64,
        fee_rate: bitcoin::Amount,
    ) -> Result<(UtxoSet, Amount)> {
        let mut utxos = utxos.to_vec();
        utxos.sort_by_key(|u| u.amount);

        let mut cur_amount = Amount::ZERO;

        // Calculate fee for head and outputs
        // 10 = virtual byte estimation of head for P2PKH legacy transaction
        // 34 = virtual byte estimation of output for P2PKH legacy transaction
        let mut cur_fee = fee_rate * (10 + 34 * output_count);

        let mut res = vec![];

        for utxo in utxos {
            // Calculate fee for new input
            // 148 = virtual byte estimation of input for P2PKH legacy transaction
            let cur_utxo_fee = fee_rate * 148;

            // Skip UTXO that costs more fee that its own value
            if utxo.amount <= cur_utxo_fee {
                continue;
            }

            cur_amount += utxo.amount;
            cur_fee += cur_utxo_fee;
            res.push(utxo);
            if cur_amount >= amount + cur_fee {
                return Ok((UtxoSet::new(res), cur_fee));
            }
        }

        bail!("not enough balance");
    }
}
