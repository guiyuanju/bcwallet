use crate::{
    btcclient::BtcClient,
    params::TransactionParams,
    receiver::{Receiver, Receivers},
    utxoset::P2PKH_OUTPUT_VBYTES,
    wallet::Wallet,
};
use anyhow::Result;
use bitcoin::{
    consensus::encode::serialize_hex,
    ecdsa::Signature as BtcSig,
    script::{self, PushBytes},
    sighash::SighashCache,
    EcdsaSighashType, TxOut,
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
    /// change is computed and appended as a receiver automatically
    pub fn prepare<T: BtcClient>(
        &self,
        client: &T,
        mut receivers: Receivers,
    ) -> Result<TransactionParams> {
        let total_out = receivers.total_out();

        // Select inputs from UTXO set that cover amount and fee
        let utxo_set = client.get_utxo_set(&self.wallet.address)?;
        let output_vbytes = receivers.output_vbytes(self.wallet.network)? + P2PKH_OUTPUT_VBYTES;
        let (inputs, fee) =
            utxo_set.select_input(total_out, output_vbytes, client.get_fee_rate()?)?;

        // Calculate change, skip if it's a dust
        let sender = &self.wallet.address;
        let dust_limit = TxOut::minimal_non_dust(sender.script_pubkey());
        let raw_change = inputs.balance() - total_out - fee;
        if raw_change >= dust_limit.value {
            receivers.push(Receiver::new(sender, raw_change));
        }

        Ok(TransactionParams::new(receivers.into_inner(), &inputs))
    }

    /// Sign a transaction from params (offline, no network access)
    /// Returns the broadcast-ready hex string
    pub fn sign(&self, params: &TransactionParams) -> Result<String> {
        let utxo_set = params.to_utxo_set();
        let mut tx = params.to_unsigned_tx(&utxo_set, self.wallet.network)?;

        let secret_key = &self.wallet.secret_key;
        let pubkey = &self.wallet.public_key;

        for (i, utxo) in utxo_set.utxos().iter().enumerate() {
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
    use crate::{
        receiver::Receivers,
        utxoset::{Utxo, UtxoSet},
    };
    use bitcoin::{Address, Amount, Network, Txid};
    use std::{cell::RefCell, str::FromStr};

    use crate::wallet::WalletFile;

    const SENDER: &str = "mwqmgMkf6ZsX2wxSK6GA2JRMVswBo29UWX";
    const RECEIVER: &str = "tb1qerzrlxcfu24davlur5sqmgzzgsal6wusda40er";

    fn stub_wallet() -> Wallet {
        WalletFile {
            private_key: "cQ7YsHdL8Spm8qv7V6weuV7MskGcF6cfZk4AaNkE1aG8nVGGjTaM".to_string(),
            address: SENDER.to_string(),
        }
        .into_wallet(Network::Testnet)
        .unwrap()
    }

    struct MockBtcClient {
        utxos: Vec<Utxo>,
        fee_rate: Amount,
        sent_txs: RefCell<Vec<String>>,
    }

    impl BtcClient for MockBtcClient {
        fn get_utxo_set(&self, _addr: &bitcoin::Address) -> anyhow::Result<UtxoSet> {
            Ok(UtxoSet::new(self.utxos.clone()))
        }
        fn get_fee_rate(&self) -> anyhow::Result<Amount> {
            Ok(self.fee_rate)
        }

        fn watch_addresses(&self, _addrs: &[&bitcoin::Address]) -> Result<()> {
            Ok(())
        }

        fn send_raw_transaction(&self, tx_hex: &str) -> anyhow::Result<Txid> {
            self.sent_txs.borrow_mut().push(tx_hex.to_string());
            Ok(
                Txid::from_str("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
                    .unwrap(),
            )
        }
    }

    fn mock_client() -> MockBtcClient {
        let sender = Address::from_str(SENDER).unwrap().assume_checked();
        MockBtcClient {
            utxos: vec![Utxo {
                txid: Txid::from_str(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .unwrap(),
                vout: 0,
                amount: Amount::from_sat(100_000),
                script_pubkey: sender.script_pubkey(),
            }],
            fee_rate: Amount::from_sat(1),
            sent_txs: RefCell::new(vec![]),
        }
    }

    #[test]
    fn test_prepare_single_receiver() {
        let tm = TransactionManager::new(stub_wallet());
        let receivers = Receivers::parse(&[(RECEIVER, 1000)], Network::Testnet).unwrap();
        let params = tm.prepare(&mock_client(), receivers).unwrap();

        assert!(!params.utxos.is_empty());
        assert!(params.receivers.len() >= 1);
        assert_eq!(params.receivers[0].amount_sat, 1000);
    }

    #[test]
    fn test_prepare_invalid_address_fails() {
        let result = Receivers::parse(&[("invalid_address", 1000)], Network::Testnet);
        assert!(result.is_err());
    }

    #[test]
    fn test_sign_produces_hex() {
        let tm = TransactionManager::new(stub_wallet());
        let receivers = Receivers::parse(&[(RECEIVER, 1000)], Network::Testnet).unwrap();
        let params = tm.prepare(&mock_client(), receivers).unwrap();

        let hex = tm.sign(&params).unwrap();
        assert!(!hex.is_empty());
        // Valid hex string (even length, all hex chars)
        assert!(hex.len() % 2 == 0);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_sign_is_deterministic() {
        let tm = TransactionManager::new(stub_wallet());
        let receivers = Receivers::parse(&[(RECEIVER, 1000)], Network::Testnet).unwrap();
        let params = tm.prepare(&mock_client(), receivers).unwrap();

        let hex1 = tm.sign(&params).unwrap();
        let hex2 = tm.sign(&params).unwrap();
        assert_eq!(hex1, hex2);
    }

    #[test]
    fn test_prepare_multiple_receivers() {
        let tm = TransactionManager::new(stub_wallet());
        let receivers =
            Receivers::parse(&[(RECEIVER, 500), (RECEIVER, 500)], Network::Testnet).unwrap();
        let params = tm.prepare(&mock_client(), receivers).unwrap();

        // At least the 2 explicit receivers
        assert!(params.receivers.len() >= 2);
        assert_eq!(params.receivers[0].amount_sat, 500);
        assert_eq!(params.receivers[1].amount_sat, 500);
    }

    #[test]
    fn test_send_broadcasts_signed_tx() {
        let tm = TransactionManager::new(stub_wallet());
        let client = mock_client();
        let receivers = Receivers::parse(&[(RECEIVER, 1000)], Network::Testnet).unwrap();
        let params = tm.prepare(&client, receivers).unwrap();

        let tx_hex = tm.sign(&params).unwrap();
        let txid = client.send_raw_transaction(&tx_hex).unwrap();

        assert_eq!(
            txid.to_string(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );
        let sent = client.sent_txs.borrow();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], tx_hex);
    }
}
