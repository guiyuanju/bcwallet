use crate::{
    btc_client::BtcClient, input_select_strategy::min::MinFirstStrategy, uxtoset::UtxoSet,
    wallet::Wallet,
};
use anyhow::Result;
use bitcoin::{Address, Amount, Transaction};

struct TransactionManager<T: BtcClient> {
    client: T,
    wallet: Wallet,
}

impl<T: BtcClient> TransactionManager<T> {
    pub fn new(client: T, wallet: Wallet) -> Self {
        Self { client, wallet }
    }

    pub fn generate_unsigned_transaction(
        &self,
        to: Address,
        amount: Amount,
        fee_rate: Amount,
    ) -> Result<Transaction> {
        let utxo_set = self.client.get_uxto_set(&self.wallet.address()?)?;
        // Fixme: calculate output count dynamically
        let inputs = utxo_set.select_input(amount, 2, fee_rate)?;
        unimplemented!()
    }

    pub fn get_fee_rate() {}

    pub fn sign() {}

    fn get_dust() -> Amount {
        unimplemented!()
    }
}
