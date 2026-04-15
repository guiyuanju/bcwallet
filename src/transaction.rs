use crate::{
    btc_client::BtcClient, input_select_strategy::min::MinFirstStrategy, uxtoset::UtxoSet,
    wallet::Wallet,
};
use anyhow::Result;
use bitcoin::{
    Address, Amount, EcdsaSighashType, Transaction, TxOut,
    absolute::LockTime,
    script::{self, PushBytes},
    sighash::SighashCache,
    transaction::Version,
};
use secp256k1::Secp256k1;

struct TransactionManager {
    wallet: Wallet,
}

impl TransactionManager {
    pub fn new(wallet: Wallet) -> Self {
        Self { wallet }
    }

    /// Generate unsigned transaction, need a Bitcoin client for UTXO query
    pub fn generate_unsigned_transaction<T: BtcClient>(
        &self,
        client: T,
        receiver: Address,
        amount_out: Amount,
    ) -> Result<Transaction> {
        // Select UTXOs based on provided strategy
        let utxo_set = client.get_uxto_set(&self.wallet.address()?)?;
        let (inputs, fee) =
            utxo_set.select_input(amount_out, 2, client.get_fee_rate()?, MinFirstStrategy())?;

        // Construct output to reciever
        let mut outputs = vec![TxOut {
            value: amount_out,
            script_pubkey: receiver.script_pubkey(),
        }];

        // Construct change to sender, skip change that is a dust
        let sender = self.wallet.address()?;
        let dust_limit = TxOut::minimal_non_dust(sender.script_pubkey());
        let amount_in = inputs.balance();
        let change = amount_in - amount_out - fee;
        if change >= dust_limit.value {
            outputs.push(TxOut {
                value: change,
                script_pubkey: sender.script_pubkey(),
            });
        }

        // Construct final unsigned transaction
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: inputs.utxos().iter().map(|u| u.into()).collect(),
            output: outputs,
        };

        Ok(tx)
    }

    /// Sign un unsigned transaction (Offline)
    pub fn sign(&self, transaction: Transaction, uxtos: UtxoSet) -> Result<Transaction> {
        let mut tx = transaction;
        let secret_key = self.wallet.secret_key()?;
        let pubkey = self.wallet.public_key()?;
        let secp = Secp256k1::new();

        for (i, utxo) in uxtos.utxos().iter().enumerate() {
            let cache = SighashCache::new(&tx);

            // Compute sig hash for current input
            let sighash = cache.legacy_signature_hash(
                i,
                utxo.script_pubkey.as_script(),
                EcdsaSighashType::All.to_u32(),
            )?;

            // Sign sig hash
            let msg = secp256k1::Message::from(sighash);
            let sig = secp.sign_ecdsa(&msg, &secret_key);

            // Serialize sig as DER bytes
            let sig_der = sig.serialize_der();
            let sig_der_bytes: &PushBytes = <&PushBytes>::try_from(sig_der.as_ref())?;

            // Build scriptSig
            let script_sig = script::Builder::new()
                .push_slice(sig_der_bytes) // signature
                .push_key(&pubkey) // public key
                .into_script();

            // Attach to tx input
            tx.input[i].script_sig = script_sig;
        }

        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use super::*;
    use crate::utils::{load_wallet, new_rpc_client};

    #[test]
    fn test_gen_transaction() {
        let wallet = load_wallet();
        let tm = TransactionManager::new(wallet);
        let receiver = Address::from_str("tb1qerzrlxcfu24davlur5sqmgzzgsal6wusda40er").unwrap();
        let receiver = receiver.require_network(bitcoin::Network::Testnet).unwrap();
        let tx = tm
            .generate_unsigned_transaction(
                new_rpc_client(),
                receiver,
                Amount::from_btc(0.00001).unwrap(),
            )
            .unwrap();

        print!("{:?}", tx);
    }

    #[test]
    fn test_sign_transaction() {
        let wallet = load_wallet();
        let tm = TransactionManager::new(wallet);
        let receiver = Address::from_str("tb1qerzrlxcfu24davlur5sqmgzzgsal6wusda40er").unwrap();
        let receiver = receiver.require_network(bitcoin::Network::Testnet).unwrap();
        let tx = tm
            .generate_unsigned_transaction(
                new_rpc_client(),
                receiver,
                Amount::from_btc(0.00001).unwrap(),
            )
            .unwrap();

        // tm.sign(tx, uxtos);
    }
}
