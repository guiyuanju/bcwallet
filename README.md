# BcWallet

**Execution result**

```bash
# Generate unsigned transaction (online)
> bcwallet prepare --receiver tb1qerzrlxcfu24davlur5sqmgzzgsal6wusda40er:1000
Params written to params.json

# Sign transaction (offline)
> bcwallet sign params.json
0200000001124f96d974d422841c1cc0ab4a7bb1c0f0b3ae13bf93b65bdb565db989ac2479000000006b4830450221009495f05935d100a35e18249aa6f09cdecd4bffa6b4838516528cdd434140a530022046229ce8e363bd4fafb22d17d3b4e96945e230dc9bf5dc54e3456c635609ec600121026c82eb6946f85ca606da46b03f2211a7e206ff4719ee699e32d6d58d9ecf6923ffffffff02e803000000000000160014c8c43f9b09e2aadeb3fc1d200da042443bfd3b9069970200000000001976a914b3110df342d2dceb87ef5a00134d34e4048e27cf88ac00000000

# Send transaction (online)
> bcwallet send 0200000001124f96d974d422841c1cc0ab4a7bb1c0f0b3ae13bf93b65bdb565db989ac2479000000006b4830450221009495f05935d100a35e18249aa6f09cdecd4bffa6b4838516528cdd434140a530022046229ce8e363bd4fafb22d17d3b4e96945e230dc9bf5dc54e3456c635609ec600121026c82eb6946f85ca606da46b03f2211a7e206ff4719ee699e32d6d58d9ecf6923ffffffff02e803000000000000160014c8c43f9b09e2aadeb3fc1d200da042443bfd3b9069970200000000001976a914b3110df342d2dceb87ef5a00134d34e4048e27cf88ac00000000
4bb0c31d2bf42158ae8114bc6b096afee0d101f709598492035c64b95b23564d
```

**Confirmed transaction result in Bitcoin testnet**

- https://mempool.space/testnet/tx/4bb0c31d2bf42158ae8114bc6b096afee0d101f709598492035c64b95b23564d
- (same transaction, alternative website) https://blockstream.info/testnet/tx/4bb0c31d2bf42158ae8114bc6b096afee0d101f709598492035c64b95b23564d

## Local Development

**prepare, send, balance and watch commands need a running Bitcoin Core RPC**.

Run

```
export BTC_RPC_PORT=<port> # default to 18332
export BTC_RPC_USER=<user> # default to user
export BTC_RPC_PASS=<pass> # default to passwd
cargo run -- <command>
```

Test

```
cargo test
```

## Usage

```bash
Usage: bcwallet <COMMAND>

Commands:
  new-wallet  Generate a new wallet file
  balance     Get balance of current address
  watch       Watch current address, need only called once for each new address
  prepare     Prepare transaction params file (online, requires RPC)
  sign        Sign a transaction from params file (offline, no network)
  send        Send a signed transaction hex to the network
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## Limitations

- Sender address only support legacy P2PKH
- Doesn't support Multi-Sig
- Uses a simple smallest UTXO first strategy to select inputs
- Transaction fee may be a little higher (diff <= sizeof(one output))

## Todo

- [x] Receiver should verify network at construction time, only once
- [ ] Separate private key and address to different files, use only address for online operations, for better security
- ~~[ ] BtcClient mock in test may panic if send concurrently, add lock~~
- [ ] Use Transaction type provided by bitcoin-core to estimate vbytes
- [ ] Multi-round UTXO output selection algorithm
- [ ] Support SegWit address
- [ ] Use PSBT to support Multi-Sig
- [ ] Allow to watch an address which has old transactions: add timestamp param to watch command

## What I learned

### Cryptocurrency

- UTXO
- Transaction: input, output, scriptSig, scriptPubKey
- Fee, dust
- Script language
- Signature: ECDSA
- Transaction format: P2PKH, P2SH, P2WPKH, P2WSH
- Encoding: DER, Base58, WIF
- Time locking
- Bitcoin Core RPC
- PSBT
- HD Wallet

### Rust

Libraries:

- bitcoin
- bitcoincore-rpc
- secp256k1

Techniques:

- Parse, don't validate: ReceiverChecked -> Receiver
- Use trait to extrait common behavior, and elimnate thin wrapper type: Receivers -> valued trait
- AI agent coding workflow for a new domain: Learn concept first -> write code manually -> build connection between code and concept -> then use AI to generate code -> read the generated code, reasom about them at the level of concept rather than code
  - Why writing code manually is necessary: By writing code, you learn the connection between a domain knowledge and the code shape, then you can read and extract knowledge from the code AI generated quickly
