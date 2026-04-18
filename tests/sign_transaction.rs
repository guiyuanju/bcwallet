use bcwallet::{
    btcclient::BtcClient,
    params::Receiver,
    transaction::TransactionManager,
    utxo::{SmallestFirst, Utxo},
    wallet::Wallet,
};
use bitcoin::{consensus::encode::deserialize_hex, Address, Amount, Network, Transaction, Txid};
use std::{cell::RefCell, str::FromStr};

struct MockBtcClient {
    utxos: Vec<Utxo>,
    fee_rate: Amount,
    sent_txs: RefCell<Vec<String>>,
}

impl BtcClient for MockBtcClient {
    fn get_utxos(&self, _addr: &Address) -> anyhow::Result<Vec<Utxo>> {
        Ok(self.utxos.clone())
    }
    fn get_fee_rate(&self) -> anyhow::Result<Amount> {
        Ok(self.fee_rate)
    }
    fn watch_addresses(&self, _addrs: &[&Address]) -> anyhow::Result<()> {
        Ok(())
    }
    fn send_raw_transaction(&self, tx_hex: &str) -> anyhow::Result<Txid> {
        self.sent_txs.borrow_mut().push(tx_hex.to_string());
        let tx: Transaction = deserialize_hex(tx_hex)?;
        Ok(tx.compute_txid())
    }
}

/// Golden test: replay the full prepare → sign → send flow using the exact
/// UTXO and receiver from the confirmed testnet transaction.
/// https://mempool.space/testnet/tx/4bb0c31d2bf42158ae8114bc6b096afee0d101f709598492035c64b95b23564d
#[test]
fn test_sign_real_confirmed_transaction() {
    let pk = "cQ7YsHdL8Spm8qv7V6weuV7MskGcF6cfZk4AaNkE1aG8nVGGjTaM";
    let sender = "mwqmgMkf6ZsX2wxSK6GA2JRMVswBo29UWX";
    let receiver = "tb1qerzrlxcfu24davlur5sqmgzzgsal6wusda40er";

    // Setup: mock client with the real UTXO from the confirmed transaction
    let addr = Address::from_str(sender).unwrap().assume_checked();
    let utxo = Utxo::new(
        "7924ac89b95d56db5bb693bf13aeb3f0c0b17b4aabc01c1c8422d474d9964f12",
        0,
        171056,
        addr,
    )
    .unwrap();
    let client = MockBtcClient {
        utxos: vec![utxo],
        fee_rate: Amount::from_sat(1),
        sent_txs: RefCell::new(vec![]),
    };

    let tm = TransactionManager::new(Wallet::new(pk, sender, Network::Testnet).unwrap());

    // 1. Prepare (online): select UTXOs and compute change
    let params = tm
        .prepare(
            &client,
            vec![Receiver::from_raw(receiver, 1000, Network::Testnet).unwrap()],
            &SmallestFirst,
        )
        .unwrap();

    assert_eq!(params.receivers.len(), 2);
    assert_eq!(params.receivers[0].amount, Amount::from_sat(1000));
    assert_eq!(params.receivers[1].amount, Amount::from_sat(169833));
    assert_eq!(params.utxos.len(), 1);

    // 2. Sign (offline): produce broadcast-ready hex
    let hex = tm.sign(&params).unwrap();

    let expected_hex = "0200000001124f96d974d422841c1cc0ab4a7bb1c0f0b3ae13bf93b65bdb565db989ac2479000000006b4830450221009495f05935d100a35e18249aa6f09cdecd4bffa6b4838516528cdd434140a530022046229ce8e363bd4fafb22d17d3b4e96945e230dc9bf5dc54e3456c635609ec600121026c82eb6946f85ca606da46b03f2211a7e206ff4719ee699e32d6d58d9ecf6923ffffffff02e803000000000000160014c8c43f9b09e2aadeb3fc1d200da042443bfd3b9069970200000000001976a914b3110df342d2dceb87ef5a00134d34e4048e27cf88ac00000000";
    assert_eq!(hex, expected_hex);

    // 3. Send (online): broadcast signed transaction
    let txid = client.send_raw_transaction(&hex).unwrap();
    assert_eq!(
        txid.to_string(),
        "4bb0c31d2bf42158ae8114bc6b096afee0d101f709598492035c64b95b23564d"
    );
}
