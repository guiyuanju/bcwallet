use crate::utxoset::{Utxo, UtxoSet};
use anyhow::{Context, Result, bail};
use bitcoin::{Address, Amount};
use bitcoincore_rpc::{
    Auth, Client, RpcApi,
    json::{ImportDescriptors, Timestamp},
};

pub trait BtcClient {
    fn get_utxo_set(&self, addr: &Address) -> Result<UtxoSet>;
    fn get_balance(&self, addr: &Address) -> Result<Amount>;
    fn get_fee_rate(&self) -> Result<Amount>;
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

    /// Import address descriptors so Bitcoin Core watches them.
    pub fn watch_address(self, addrs: &[&Address]) -> Result<()> {
        for &addr in addrs {
            self.import_descriptor(addr)?;
        }
        Ok(())
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
    fn get_utxo_set(&self, addr: &Address) -> Result<UtxoSet> {
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
        Ok(self.get_utxo_set(addr)?.balance())
    }

    fn get_fee_rate(&self) -> Result<Amount> {
        let res = self
            .client
            .estimate_smart_fee(1, None)
            .context("failed to get smart fee")?;
        if let Some(errs) = res.errors {
            bail!("failed to get smart fee: {:?}", errs);
        }
        if let Some(fee_rate) = res.fee_rate {
            // estimatesmartfee returns BTC/kB, convert to sat/vB
            return Ok(Amount::from_sat(fee_rate.to_sat() / 1000));
        }
        bail!("failed to get smart fee");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::Wallet;
    use bitcoin::Network;
    use std::{env, sync::LazyLock};

    fn load_wallet() -> Wallet {
        let path = env::var("WALLET").unwrap_or("wallet.json".to_owned());
        Wallet::from_file(&path, Network::Testnet).unwrap()
    }

    fn new_rpc_client() -> LocalRpc {
        let port = env::var("BTC_RPC_PORT").unwrap_or("18332".to_owned());
        let user = env::var("BTC_RPC_USER").unwrap_or("user".to_owned());
        let pass = env::var("BTC_RPC_PASS").unwrap_or("passwd".to_owned());
        LocalRpc::new(&port, &user, &pass).unwrap()
    }

    struct Cache {
        addr: Address,
        client: LocalRpc,
    }

    static CACHE: LazyLock<Cache> = LazyLock::new(|| {
        let wallet = load_wallet();
        let addr = wallet.address().clone();
        let client = new_rpc_client();
        Cache { addr, client }
    });

    #[test]
    fn test_get_utxos() {
        let res = CACHE.client.get_utxo_set(&CACHE.addr).unwrap();
        println!("{:?}", res.utxos());
    }
}
