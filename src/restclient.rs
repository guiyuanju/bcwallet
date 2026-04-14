use crate::{network::NetworkClient, types::Satoshi};
use anyhow::{Context, Result, bail};
use bitcoin::Address;
use bitcoincore_rpc::{
    Auth, Client, RpcApi,
    json::{ImportDescriptors, ListUnspentResultEntry, Timestamp},
};

/// The client used to communicate with local Bitcoin Core node.
pub struct LocalRpcClient {
    client: Client,
}

impl LocalRpcClient {
    // Security concern: password is stored in memory (client).
    pub fn new(port: &str, username: &str, passwd: &str) -> Result<Self> {
        let url = format!("http://127.0.0.1:{port}");
        let auth = Auth::UserPass(username.to_owned(), passwd.to_owned());
        let client = Client::new(&url, auth).context("faled to connet to local rpc client")?;

        Ok(Self { client })
    }

    /// Set the address to scan and watch, consumes itself, returns a DescriptorLocalRpcClient
    /// which enables wallet related operations like get_balance.
    pub fn watch_address(self, addrs: &[&Address]) -> Result<DescriptorLocalRpcClient> {
        for &addr in addrs {
            self.import_descriptor(addr)?;
        }

        Ok(DescriptorLocalRpcClient::new(self.client))
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

/// The client which ensures the descriptor of the address has been imported and scanned.
pub struct DescriptorLocalRpcClient {
    client: Client,
}

impl DescriptorLocalRpcClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl NetworkClient for DescriptorLocalRpcClient {
    fn get_uxtos(&self, addr: &Address) -> Result<Vec<ListUnspentResultEntry>> {
        // get all transactions confirmed by at least one block
        self.client
            .list_unspent(Some(1), None, Some(&[addr]), Some(false), None)
            .context("failed to list unspent")
    }

    fn get_balance(&self, addr: &Address) -> Result<Satoshi> {
        let utxos = self.get_uxtos(addr)?;
        Ok(utxos.iter().map(|e| e.amount.to_sat()).sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::Wallet;

    fn get_addr() -> Address {
        let mut wallet = Wallet::new();
        wallet.load("wallet_test.json").unwrap();
        wallet.address().unwrap()
    }

    #[test]
    fn test_import_descriptors() {
        let addr = get_addr();
        let client = LocalRpcClient::new("18332", "user", "passwd").unwrap();
        assert!(client.import_descriptor(&addr).is_ok());
    }

    // #[test]
    // fn test_get_utxos() {
    //     let addr = get_addr();
    //     let client = client.watch_address(&[&addr]).unwrap();
    //     let res = client.get_uxtos(&addr).unwrap();
    //     println!("{:?}", res);
    // }

    // #[test]
    // fn test_json() {
    //     let mut wallet = Wallet::new();
    //     wallet.load("wallet_test.json").unwrap();
    //     let addr = wallet.address().unwrap();
    //     let descriptor = ImportDescriptors {
    //         descriptor: addr.to_string(),
    //         timestamp: bitcoincore_rpc::json::Timestamp::Now, // scan from now
    //         active: Some(false),                              // watch only
    //         range: None,                                      // fixed single address
    //         next_index: None,                                 // since no auto address generation
    //         internal: Some(false),                            // receive payment from others
    //         label: Some("bcwallet".to_owned()),
    //     };
    //     let json_request = vec![serde_json::to_value(descriptor).unwrap()];
    //     println!("{:?}", json_request);
    // }
}
