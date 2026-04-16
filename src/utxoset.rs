use anyhow::{bail, Context, Result};
use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, TxIn, Txid, Witness};
use bitcoincore_rpc::json::ListUnspentResultEntry;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::valued::Valued;

/// Estimated vbytes for a legacy P2PKH input (script_sig: push sig + push pubkey).
pub const P2PKH_INPUT_VBYTES: u64 = 148;
/// Estimated vbytes for a legacy P2PKH output (8 value + 1 script_len + 25 script).
pub const P2PKH_OUTPUT_VBYTES: u64 = 34;
/// Overhead vbytes for a transaction (version + locktime + input/output counts).
const TX_OVERHEAD_VBYTES: u64 = 10;

/// Custom Utxo type to decouple from RPC client implementations.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(into = "UtxoParam", try_from = "UtxoParam")]
pub struct Utxo {
    pub txid: Txid,
    pub vout: u32,
    pub amount: Amount,
    pub script_pubkey: ScriptBuf,
}

impl Valued for Utxo {
    fn value(&self) -> Amount {
        self.amount
    }
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

/// Strategy for selecting UTXOs to fund a transaction.
pub trait CoinSelector {
    /// Select UTXOs from `utxos` that cover `target` amount plus estimated fees.
    /// Returns (selected UTXOs, estimated fee).
    fn select(
        &self,
        utxos: &[Utxo],
        target: Amount,
        output_vbytes: u64,
        fee_rate: Amount,
    ) -> Result<(Vec<Utxo>, Amount)>;
}

/// Selects the smallest UTXOs first, skipping dust.
pub struct SmallestFirst;

impl CoinSelector for SmallestFirst {
    fn select(
        &self,
        utxos: &[Utxo],
        target: Amount,
        output_vbytes: u64,
        fee_rate: Amount,
    ) -> Result<(Vec<Utxo>, Amount)> {
        let mut utxos = utxos.to_vec();
        utxos.sort_by_key(|u| u.amount);

        let mut cur_amount = Amount::ZERO;
        let mut cur_fee = fee_rate * (TX_OVERHEAD_VBYTES + output_vbytes);
        let mut selected = vec![];

        for utxo in utxos {
            let input_fee = fee_rate * P2PKH_INPUT_VBYTES;

            // Skip UTXOs that cost more in fees than they're worth
            if utxo.amount <= input_fee {
                continue;
            }

            cur_amount += utxo.amount;
            cur_fee += input_fee;
            selected.push(utxo);

            if cur_amount >= target + cur_fee {
                return Ok((selected, cur_fee));
            }
        }

        bail!("not enough balance");
    }
}

/// Private serialization proxy for [`Utxo`].
#[derive(Serialize, Deserialize)]
struct UtxoParam {
    txid: String,
    vout: u32,
    amount_sat: u64,
    script_pubkey: String, // hex-encoded
}

impl From<Utxo> for UtxoParam {
    fn from(utxo: Utxo) -> Self {
        Self {
            txid: utxo.txid.to_string(),
            vout: utxo.vout,
            amount_sat: utxo.amount.to_sat(),
            script_pubkey: utxo.script_pubkey.to_hex_string(),
        }
    }
}

impl TryFrom<UtxoParam> for Utxo {
    type Error = anyhow::Error;

    fn try_from(p: UtxoParam) -> Result<Self> {
        Ok(Self {
            txid: Txid::from_str(&p.txid).context("invalid txid")?,
            vout: p.vout,
            amount: Amount::from_sat(p.amount_sat),
            script_pubkey: ScriptBuf::from_hex(&p.script_pubkey)
                .context("invalid script_pubkey hex")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valued::ValuedSlice;
    use bitcoin::address::Address;

    const TXID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn utxo(sat: u64) -> Utxo {
        let addr = Address::from_str("mwqmgMkf6ZsX2wxSK6GA2JRMVswBo29UWX")
            .unwrap()
            .assume_checked();
        Utxo {
            txid: Txid::from_str(TXID).unwrap(),
            vout: 0,
            amount: Amount::from_sat(sat),
            script_pubkey: addr.script_pubkey(),
        }
    }

    #[test]
    fn test_select_single_utxo_covers_amount() {
        let utxos = vec![utxo(100_000)];
        let (selected, fee) = SmallestFirst
            .select(
                &utxos,
                Amount::from_sat(1_000),
                P2PKH_OUTPUT_VBYTES,
                Amount::from_sat(1),
            )
            .unwrap();

        assert_eq!(selected.len(), 1);
        assert!(fee > Amount::ZERO);
        assert!(selected.total_value() >= Amount::from_sat(1_000) + fee);
    }

    #[test]
    fn test_select_multiple_utxos_when_one_not_enough() {
        let utxos = vec![utxo(5_000), utxo(5_000), utxo(5_000)];
        let (selected, fee) = SmallestFirst
            .select(
                &utxos,
                Amount::from_sat(9_000),
                P2PKH_OUTPUT_VBYTES,
                Amount::from_sat(1),
            )
            .unwrap();

        assert!(selected.len() >= 2);
        assert!(selected.total_value() >= Amount::from_sat(9_000) + fee);
    }

    #[test]
    fn test_select_skips_dust_utxos() {
        // A UTXO worth less than the fee to spend it (148 sat at 1 sat/vB) should be skipped
        let utxos = vec![utxo(100), utxo(100_000)];
        let (selected, _fee) = SmallestFirst
            .select(
                &utxos,
                Amount::from_sat(1_000),
                P2PKH_OUTPUT_VBYTES,
                Amount::from_sat(1),
            )
            .unwrap();

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].amount, Amount::from_sat(100_000));
    }

    #[test]
    fn test_select_fails_on_insufficient_balance() {
        let utxos = vec![utxo(500)];
        let result = SmallestFirst.select(
            &utxos,
            Amount::from_sat(100_000),
            P2PKH_OUTPUT_VBYTES,
            Amount::from_sat(1),
        );

        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected error"),
        };
        assert!(err.to_string().contains("not enough balance"));
    }

    #[test]
    fn test_select_fails_on_empty_set() {
        let result = SmallestFirst.select(
            &[],
            Amount::from_sat(1_000),
            P2PKH_OUTPUT_VBYTES,
            Amount::from_sat(1),
        );

        assert!(matches!(result, Err(_)));
    }

    #[test]
    fn test_select_all_dust_fails() {
        // All UTXOs cost more to spend than they're worth
        let utxos = vec![utxo(100), utxo(50), utxo(148)];
        let result = SmallestFirst.select(
            &utxos,
            Amount::from_sat(1_000),
            P2PKH_OUTPUT_VBYTES,
            Amount::from_sat(1),
        );

        assert!(matches!(result, Err(_)));
    }
}
