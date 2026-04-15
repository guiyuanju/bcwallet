use crate::{
    btc_client::BtcClient,
    input_select_strategy::min::MinFirstStrategy,
    params::{Receiver, Receivers, TransactionParams},
    wallet::Wallet,
};
use anyhow::Result;
use bitcoin::{
    EcdsaSighashType, Network, TxOut,
    consensus::encode::serialize_hex,
    ecdsa::Signature as BtcSig,
    script::{self, PushBytes},
    sighash::SighashCache,
};
use secp256k1::Secp256k1;

pub struct TransactionManager {
    wallet: Wallet,
    network: Network,
}

impl TransactionManager {
    pub fn new(wallet: Wallet, network: Network) -> Self {
        Self { wallet, network }
    }

    /// Prepare transaction params by fetching UTXOs online,
    /// change is computed and appended as a receiver automatically
    pub fn prepare<T: BtcClient>(
        &self,
        client: T,
        mut receivers: Receivers,
    ) -> Result<TransactionParams> {
        let total_out = receivers.total_out();

        // Select inputs from UTXO set that cover amount and fee
        let utxo_set = client.get_utxo_set(&self.wallet.address(self.network)?)?;
        // P2PKH change output = 34 vbytes
        let output_vbytes = receivers.output_vbytes(self.network)? + 34;
        let (inputs, fee) = utxo_set.select_input(
            total_out,
            output_vbytes,
            client.get_fee_rate()?,
            MinFirstStrategy(),
        )?;

        // Calculate change, skip if it's a dust
        let sender = self.wallet.address(self.network)?;
        let dust_limit = TxOut::minimal_non_dust(sender.script_pubkey());
        let raw_change = inputs.balance() - total_out - fee;
        if raw_change >= dust_limit.value {
            receivers.push(Receiver::new(&sender, raw_change));
        }

        Ok(TransactionParams::new(receivers.into_inner(), &inputs))
    }

    /// Sign a transaction from params (offline, no network access).
    /// Returns the broadcast-ready hex string.
    pub fn sign(&self, params: &TransactionParams) -> Result<String> {
        let utxo_set = params.to_utxo_set()?;
        let mut tx = params.to_unsigned_tx(&utxo_set, self.network)?;

        let secret_key = self.wallet.secret_key()?;
        let pubkey = self.wallet.public_key()?;
        let secp = Secp256k1::new();

        for (i, utxo) in utxo_set.utxos().iter().enumerate() {
            // Compute the legacy sighash for this input (hash of tx + input's scriptPubKey)
            let cache = SighashCache::new(&tx);
            let sighash = cache.legacy_signature_hash(
                i,
                utxo.script_pubkey.as_script(),
                EcdsaSighashType::All.to_u32(),
            )?;

            // ECDSA sign, then wrap with sighash type so serialization is DER + 0x01
            let sig = secp.sign_ecdsa(&secp256k1::Message::from(sighash), &secret_key);
            let btc_sig = BtcSig { signature: sig, sighash_type: EcdsaSighashType::All };
            let serialized = btc_sig.serialize();
            let sig_bytes: &PushBytes = serialized.as_ref();

            // Build P2PKH scriptSig: <signature> <pubkey>
            tx.input[i].script_sig = script::Builder::new()
                .push_slice(sig_bytes)
                .push_key(&pubkey)
                .into_script();
        }

        Ok(serialize_hex(&tx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        params::Receivers,
        utxoset::{Utxo, UtxoSet},
        utils::load_wallet,
    };
    use bitcoin::{Amount, Txid};
    use std::str::FromStr;

    const RECEIVER: &str = "tb1qerzrlxcfu24davlur5sqmgzzgsal6wusda40er";

    struct MockBtcClient {
        utxos: Vec<Utxo>,
        fee_rate: Amount,
    }

    impl BtcClient for MockBtcClient {
        fn get_utxo_set(&self, _addr: &bitcoin::Address) -> anyhow::Result<UtxoSet> {
            Ok(UtxoSet::new(self.utxos.clone()))
        }
        fn get_balance(&self, _addr: &bitcoin::Address) -> anyhow::Result<Amount> {
            Ok(UtxoSet::new(self.utxos.clone()).balance())
        }
        fn get_fee_rate(&self) -> anyhow::Result<Amount> {
            Ok(self.fee_rate)
        }
    }

    fn mock_client() -> MockBtcClient {
        let wallet = load_wallet();
        let addr = wallet.address(Network::Testnet).unwrap();
        MockBtcClient {
            utxos: vec![Utxo {
                txid: Txid::from_str(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .unwrap(),
                vout: 0,
                amount: Amount::from_sat(100_000),
                script_pubkey: addr.script_pubkey(),
            }],
            fee_rate: Amount::from_sat(1), // 1 sat/vB
        }
    }

    #[test]
    fn test_prepare_single_receiver() {
        let tm = TransactionManager::new(load_wallet(), Network::Testnet);
        let receivers = Receivers::parse(&[(RECEIVER, 1000)], Network::Testnet).unwrap();
        let params = tm.prepare(mock_client(), receivers).unwrap();

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
        let tm = TransactionManager::new(load_wallet(), Network::Testnet);
        let receivers = Receivers::parse(&[(RECEIVER, 1000)], Network::Testnet).unwrap();
        let params = tm.prepare(mock_client(), receivers).unwrap();

        let hex = tm.sign(&params).unwrap();
        assert!(!hex.is_empty());
        // Valid hex string (even length, all hex chars)
        assert!(hex.len() % 2 == 0);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_sign_is_deterministic() {
        let tm = TransactionManager::new(load_wallet(), Network::Testnet);
        let receivers = Receivers::parse(&[(RECEIVER, 1000)], Network::Testnet).unwrap();
        let params = tm.prepare(mock_client(), receivers).unwrap();

        let hex1 = tm.sign(&params).unwrap();
        let hex2 = tm.sign(&params).unwrap();
        assert_eq!(hex1, hex2);
    }

    #[test]
    fn test_prepare_multiple_receivers() {
        let tm = TransactionManager::new(load_wallet(), Network::Testnet);
        let receivers =
            Receivers::parse(&[(RECEIVER, 500), (RECEIVER, 500)], Network::Testnet).unwrap();
        let params = tm.prepare(mock_client(), receivers).unwrap();

        // At least the 2 explicit receivers
        assert!(params.receivers.len() >= 2);
        assert_eq!(params.receivers[0].amount_sat, 500);
        assert_eq!(params.receivers[1].amount_sat, 500);
    }
}
