//! The core of generate and sign unsigned transactions.

use crate::{
    btcclient::BtcClient,
    params::{output_vbytes, Receiver, TransactionParam},
    utxo::{CoinSelector, P2PKH_OUTPUT_VBYTES},
    wallet::Wallet,
};
use anyhow::Result;
use bitcoin::{
    consensus::encode::serialize_hex,
    ecdsa::Signature as BtcSig,
    script::{self, PushBytes},
    sighash::SighashCache,
    Amount, EcdsaSighashType, Transaction, TxOut,
};
use secp256k1::Secp256k1;

pub struct TransactionManager {
    wallet: Wallet,
    secp: Secp256k1<secp256k1::All>,
}

impl TransactionManager {
    pub fn new(wallet: Wallet) -> Self {
        Self {
            wallet,
            secp: Secp256k1::new(),
        }
    }

    /// Prepare transaction params by fetching UTXOs online,
    /// change is computed and appended as a receiver automatically.
    pub fn prepare<T: BtcClient>(
        &self,
        client: &T,
        mut receivers: Vec<Receiver>,
        selector: &dyn CoinSelector,
    ) -> Result<TransactionParam> {
        let total_out: Amount = receivers.iter().map(|r| r.amount).sum();

        // Select inputs that cover amount and fee
        let utxos = client.get_utxos(&self.wallet.address)?;
        let output_vbytes = output_vbytes(&receivers) + P2PKH_OUTPUT_VBYTES;
        let (selected, fee) =
            selector.select(&utxos, total_out, output_vbytes, client.get_fee_rate()?)?;

        // Calculate change, skip if it's dust
        let sender = &self.wallet.address;
        let dust_limit = TxOut::minimal_non_dust(sender.script_pubkey());
        let raw_change: Amount =
            selected.iter().map(|u| u.amount).sum::<Amount>() - total_out - fee;
        if raw_change >= dust_limit.value {
            receivers.push(Receiver::new(sender.clone(), raw_change));
        }

        Ok(TransactionParam::new(receivers, selected))
    }

    /// Sign a transaction from params (offline, no network access),
    /// returns the broadcast-ready hex string.
    pub fn sign(&self, params: &TransactionParam) -> Result<String> {
        let mut tx: Transaction = params.into();

        let secret_key = &self.wallet.secret_key;
        let pubkey = &self.wallet.public_key;

        for (i, utxo) in params.utxos.iter().enumerate() {
            // Compute the legacy sighash for this input
            let cache = SighashCache::new(&tx);
            let sighash = cache.legacy_signature_hash(
                i,
                utxo.script_pubkey.as_script(),
                EcdsaSighashType::All.to_u32(),
            )?;

            // ECDSA sign and serialize to DER + SighashType
            let sig = self
                .secp
                .sign_ecdsa(&secp256k1::Message::from(sighash), secret_key);
            let btc_sig = BtcSig {
                signature: sig,
                sighash_type: EcdsaSighashType::All,
            };
            let serialized = btc_sig.serialize();
            let sig_bytes: &PushBytes = serialized.as_ref();

            // Build P2PKH scriptSig <signature> <pubkey>
            tx.input[i].script_sig = script::Builder::new()
                .push_slice(sig_bytes)
                .push_key(pubkey)
                .into_script();
        }

        Ok(serialize_hex(&tx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utxo::{SmallestFirst, Utxo};
    use bitcoin::{absolute::Time, Address, Amount, Network, Txid};
    use std::{cell::RefCell, str::FromStr};

    const PK: &str = "cQ7YsHdL8Spm8qv7V6weuV7MskGcF6cfZk4AaNkE1aG8nVGGjTaM";
    const SENDER: &str = "mwqmgMkf6ZsX2wxSK6GA2JRMVswBo29UWX";
    const RECEIVER: &str = "tb1qerzrlxcfu24davlur5sqmgzzgsal6wusda40er";
    const INPUT_TX_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OUTPUT_TX_ID: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    struct MockBtcClient {
        utxos: Vec<Utxo>,
        fee_rate: Amount,
        sent_txs: RefCell<Vec<String>>,
    }

    impl BtcClient for MockBtcClient {
        fn get_utxos(&self, _addr: &bitcoin::Address) -> anyhow::Result<Vec<Utxo>> {
            Ok(self.utxos.clone())
        }
        fn get_fee_rate(&self) -> anyhow::Result<Amount> {
            Ok(self.fee_rate)
        }

        fn watch_addresses(&self, _addrs: &[&bitcoin::Address], _from: &[&Time]) -> Result<()> {
            Ok(())
        }

        fn send_raw_transaction(&self, tx_hex: &str) -> anyhow::Result<Txid> {
            self.sent_txs.borrow_mut().push(tx_hex.to_string());
            Ok(Txid::from_str(OUTPUT_TX_ID).unwrap())
        }
    }

    fn mock_client() -> MockBtcClient {
        let addr = Address::from_str(SENDER).unwrap().assume_checked();
        let utxo = Utxo::new(INPUT_TX_ID, 0, 100_000, addr).unwrap();

        MockBtcClient {
            utxos: vec![utxo],
            fee_rate: Amount::from_sat(1),
            sent_txs: RefCell::new(vec![]),
        }
    }

    #[test]
    fn test_prepare_single_receiver() {
        let tm = TransactionManager::new(Wallet::new(PK, SENDER, Network::Testnet).unwrap());
        let params = tm
            .prepare(
                &mock_client(),
                vec![Receiver::from_raw(RECEIVER, 1000, Network::Testnet).unwrap()],
                &SmallestFirst,
            )
            .unwrap();

        assert!(!params.utxos.is_empty());
        assert!(params.receivers.len() >= 1);
        assert_eq!(params.receivers[0].amount, Amount::from_sat(1000));
    }

    #[test]
    fn test_prepare_multiple_receivers() {
        let tm = TransactionManager::new(Wallet::new(PK, SENDER, Network::Testnet).unwrap());
        let params = tm
            .prepare(
                &mock_client(),
                vec![
                    Receiver::from_raw(RECEIVER, 500, Network::Testnet).unwrap(),
                    Receiver::from_raw(RECEIVER, 500, Network::Testnet).unwrap(),
                ],
                &SmallestFirst,
            )
            .unwrap();

        assert!(params.receivers.len() >= 2);
        assert_eq!(params.receivers[0].amount, Amount::from_sat(500));
        assert_eq!(params.receivers[1].amount, Amount::from_sat(500));
    }

    #[test]
    fn test_sign_produces_hex() {
        let tm = TransactionManager::new(Wallet::new(PK, SENDER, Network::Testnet).unwrap());
        let params = tm
            .prepare(
                &mock_client(),
                vec![Receiver::from_raw(RECEIVER, 1000, Network::Testnet).unwrap()],
                &SmallestFirst,
            )
            .unwrap();

        let hex = tm.sign(&params).unwrap();
        assert!(!hex.is_empty());
        assert!(hex.len() % 2 == 0);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_sign_is_deterministic() {
        let tm = TransactionManager::new(Wallet::new(PK, SENDER, Network::Testnet).unwrap());
        let params = tm
            .prepare(
                &mock_client(),
                vec![Receiver::from_raw(RECEIVER, 1000, Network::Testnet).unwrap()],
                &SmallestFirst,
            )
            .unwrap();

        let hex1 = tm.sign(&params).unwrap();
        let hex2 = tm.sign(&params).unwrap();
        assert_eq!(hex1, hex2);
    }
}
