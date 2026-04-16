//! Integration tests for the prepare → sign → send workflow.
//!
//! These tests require a running Bitcoin Core node (e.g. regtest).
//! Set the following environment variables:
//!   BTC_RPC_PORT, BTC_RPC_USER, BTC_RPC_PASS, WALLET
//!
//! Run with: cargo test --test integration -- --ignored

use std::process::Command;

/// Helper: run the bcwallet binary with the given args.
fn bcwallet(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_bcwallet"))
        .args(args)
        .output()
        .expect("failed to execute bcwallet")
}

#[test]
#[ignore] // requires a live Bitcoin Core node
fn test_send_command_rejects_invalid_hex() {
    let output = bcwallet(&["send", "not_valid_hex"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("failed to send raw transaction"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
#[ignore] // requires a live Bitcoin Core node
fn test_prepare_sign_send_roundtrip() {
    // This test assumes:
    //   - A regtest/testnet node is running
    //   - The wallet file at $WALLET has been funded
    //   - The wallet address has been imported via `watch`
    //
    // 1. Prepare
    let params_path = "/tmp/bcwallet_integration_params.json";
    let receiver = std::env::var("TEST_RECEIVER")
        .expect("set TEST_RECEIVER=address:amount_sat for integration test");

    let output = bcwallet(&["prepare", "--receiver", &receiver, "--output", params_path]);
    assert!(
        output.status.success(),
        "prepare failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // 2. Sign
    let output = bcwallet(&["sign", params_path]);
    assert!(
        output.status.success(),
        "sign failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tx_hex = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(!tx_hex.is_empty());
    assert!(tx_hex.chars().all(|c| c.is_ascii_hexdigit()));

    // 3. Send
    let output = bcwallet(&["send", &tx_hex]);
    assert!(
        output.status.success(),
        "send failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let txid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // A valid txid is 64 hex characters
    assert_eq!(txid.len(), 64, "unexpected txid: {txid}");
    assert!(txid.chars().all(|c| c.is_ascii_hexdigit()));

    // Cleanup
    std::fs::remove_file(params_path).ok();
}
