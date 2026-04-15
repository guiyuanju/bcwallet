use anyhow::{Result, bail};
use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, TxIn, Txid, Witness};
use bitcoincore_rpc::json::ListUnspentResultEntry;

/// Custom Utxo type to decouple from RPC client implementations.
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

    /// Select the smallest UTXOs that cover `amount` + fees.
    /// Skips UTXOs whose value is less than the fee to spend them.
    /// Returns (selected UTXO set, estimated fee).
    pub fn select_input(
        &self,
        amount: Amount,
        output_vbytes: u64,
        fee_rate: Amount,
    ) -> Result<(UtxoSet, Amount)> {
        let mut utxos = self.utxos.clone();
        utxos.sort_by_key(|u| u.amount);

        let mut cur_amount = Amount::ZERO;
        // 10 vbytes for transaction header (version, locktime, etc.)
        let mut cur_fee = fee_rate * (10 + output_vbytes);
        let mut selected = vec![];

        for utxo in utxos {
            // 148 vbytes per P2PKH input
            let input_fee = fee_rate * 148;

            // Skip UTXOs that cost more in fees than they're worth
            if utxo.amount <= input_fee {
                continue;
            }

            cur_amount += utxo.amount;
            cur_fee += input_fee;
            selected.push(utxo);

            if cur_amount >= amount + cur_fee {
                return Ok((UtxoSet::new(selected), cur_fee));
            }
        }

        bail!("not enough balance");
    }

    pub fn balance(&self) -> Amount {
        Amount::from_sat(self.utxos.iter().map(|e| e.amount.to_sat()).sum())
    }

    pub fn utxos(&self) -> &[Utxo] {
        &self.utxos
    }
}
