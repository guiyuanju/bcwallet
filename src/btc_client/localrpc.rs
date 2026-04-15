use crate::{
    btc_client::BtcClient,
    uxtoset::{Utxo, UtxoSet},
};
use anyhow::{Context, Result, bail};
use bitcoin::{Address, Amount};
use bitcoincore_rpc::{
    Auth, Client, RpcApi,
    json::{ImportDescriptors, Timestamp},
};

/// The client used to communicate with local Bitcoin Core node.
pub struct LocalRpc {
    client: Client,
}

impl LocalRpc {
    pub fn new(port: &str, username: &str, passwd: &str) -> Result<Self> {
        // Security concern: password is stored in memory (client).
        let url = format!("http://127.0.0.1:{port}");
        let auth = Auth::UserPass(username.to_owned(), passwd.to_owned());
        let client = Client::new(&url, auth).context("faled to connet to local rpc client")?;

        Ok(Self { client })
    }

    /// Set the address to scan and watch.
    pub fn watch_address(self, addrs: &[&Address]) -> Result<()> {
        for &addr in addrs {
            self.import_descriptor(addr)?;
        }

        Ok(())
    }

    fn import_descriptor(&self, addr: &Address) -> Result<()> {
        // compose the correct descriptor fotmat
        let addr_descrip = format!("addr({})", addr.to_string());

        // get checksumed descriptor
        let descriptor = self
            .client
            .get_descriptor_info(&addr_descrip)
            .context("failed to get checksumed descriptor")?
            .descriptor;

        // import descriptor for bitcoin core to watch
        let mut req = ImportDescriptors::default();
        req.descriptor = descriptor;
        req.timestamp = Timestamp::Now;

        let res = self
            .client
            .import_descriptors(req)
            .context("failed to import descriptors")?;

        // check result
        for r in res {
            if !r.success {
                bail!(format!(
                    "failed to import descriptor: {:?}, {:?}",
                    r.warnings, r.error
                ))
            }
        }

        Ok(())
    }
}

impl BtcClient for LocalRpc {
    fn get_uxto_set(&self, addr: &Address) -> Result<UtxoSet> {
        // get all transactions confirmed by at least one block
        let utxos = self
            .client
            .list_unspent(Some(1), None, Some(&[addr]), Some(false), None)
            .context("failed to list unspent")?
            .iter()
            .map(|e| e.into())
            .collect::<Vec<Utxo>>();

        Ok(UtxoSet::new(utxos))
    }

    fn get_balance(&self, addr: &Address) -> Result<Amount> {
        let utxos = self.get_uxto_set(addr)?;
        Ok(utxos.balance())
    }

    fn get_fee_rate(&self) -> Result<Amount> {
        // Get conservative fee with 1 block confirmation
        let res = self
            .client
            .estimate_smart_fee(1, None)
            .context("failed to get smart fee")?;
        if let Some(errs) = res.errors {
            bail!("failed to get smart fee: {:?}", errs);
        }
        if let Some(fee_rate) = res.fee_rate {
            return Ok(fee_rate);
        }
        bail!("failed to get smart fee");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{load_wallet, new_rpc_client};
    use std::sync::LazyLock;

    struct Cache {
        addr: Address,
        client: LocalRpc,
    }

    static CACHE: LazyLock<Cache> = LazyLock::new(|| {
        let wallet = load_wallet();
        let addr = wallet.address().unwrap();
        let client = new_rpc_client();

        Cache { addr, client }
    });

    #[test]
    fn test_get_utxos() {
        let addr = &CACHE.addr;
        let client = &CACHE.client;
        let res = client.get_uxto_set(&addr).unwrap();
        println!("{:?}", res.utxos());
    }
}
