use crate::utxoset::Utxo;
use crate::valued::ValuedSlice;
use anyhow::{bail, Context, Result};
use bitcoin::{Address, Amount, Txid};
use bitcoincore_rpc::{
    json::{ImportDescriptors, Timestamp},
    Auth, Client, RpcApi,
};

pub trait BtcClient {
    fn get_utxos(&self, addr: &Address) -> Result<Vec<Utxo>>;
    fn get_balance(&self, addr: &Address) -> Result<Amount> {
        Ok(self.get_utxos(addr)?.total_value())
    }
    fn get_fee_rate(&self) -> Result<Amount>;
    fn watch_addresses(&self, addrs: &[&Address]) -> Result<()>;
    fn send_raw_transaction(&self, tx_hex: &str) -> Result<Txid>;
}

/// Client for communicating with a local Bitcoin Core node.
pub struct LocalRpc {
    client: Client,
}

impl LocalRpc {
    pub fn new(port: &str, username: &str, passwd: &str) -> Result<Self> {
        let url = format!("http://127.0.0.1:{port}");
        let auth = Auth::UserPass(username.to_owned(), passwd.to_owned());
        let client = Client::new(&url, auth).context("failed to connect to local rpc client")?;
        Ok(Self { client })
    }

    fn import_descriptor(&self, addr: &Address) -> Result<()> {
        let addr_descrip = format!("addr({addr})");

        let descriptor = self
            .client
            .get_descriptor_info(&addr_descrip)
            .context("failed to get checksumed descriptor")?
            .descriptor;

        let mut req = ImportDescriptors::default();
        req.descriptor = descriptor;
        req.timestamp = Timestamp::Now;

        let res = self
            .client
            .import_descriptors(req)
            .context("failed to import descriptors")?;

        for r in res {
            if !r.success {
                bail!(
                    "failed to import descriptor: {:?}, {:?}",
                    r.warnings,
                    r.error
                )
            }
        }

        Ok(())
    }
}

impl BtcClient for LocalRpc {
    fn get_utxos(&self, addr: &Address) -> Result<Vec<Utxo>> {
        Ok(self
            .client
            .list_unspent(Some(1), None, Some(&[addr]), Some(false), None)
            .context("failed to list unspent")?
            .iter()
            .map(|e| e.into())
            .collect())
    }

    /// Import address descriptors so Bitcoin Core watches them.
    fn watch_addresses(&self, addrs: &[&Address]) -> Result<()> {
        for &addr in addrs {
            self.import_descriptor(addr)?;
        }
        Ok(())
    }

    fn get_fee_rate(&self) -> Result<Amount> {
        let res = self
            .client
            .estimate_smart_fee(1, None)
            .context("failed to get smart fee")?;
        if let Some(errs) = res.errors {
            bail!("failed to get smart fee: {:?}", errs);
        }
        // estimatesmartfee returns BTC/kB, convert to sat/vB
        res.fee_rate
            .map(|r| Amount::from_sat(r.to_sat() / 1000))
            .context("failed to get smart fee")
    }

    fn send_raw_transaction(&self, tx_hex: &str) -> Result<Txid> {
        self.client
            .send_raw_transaction(tx_hex)
            .context("failed to send raw transaction")
    }
}
