//! Test script for bridge_usdc - Across Protocol API integration
//!
//! This tests the Across API call without requiring a real wallet.
//! Run with: cargo run --example test_bridge_usdc
//!
//! For real bridging, set BURNER_WALLET_BOT_PRIVATE_KEY env var.

use serde::Deserialize;

const ACROSS_API_URL: &str = "https://app.across.to/api";

// Chain configurations
const CHAINS: &[(&str, u64, &str)] = &[
    ("ethereum", 1, "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
    ("base", 8453, "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
    ("polygon", 137, "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"),
    ("arbitrum", 42161, "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"),
    ("optimism", 10, "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85"),
];

#[derive(Debug, Deserialize)]
struct AcrossSwapResponse {
    #[serde(rename = "approvalTxns", default)]
    approval_txns: Vec<AcrossTransaction>,
    #[serde(rename = "swapTx")]
    swap_tx: Option<AcrossSwapTx>,
    #[serde(rename = "expectedOutputAmount")]
    expected_output_amount: Option<String>,
    #[serde(rename = "expectedFillTime")]
    expected_fill_time: Option<u64>,
    #[serde(rename = "fees")]
    fees: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct AcrossSwapTx {
    to: String,
    data: String,
    #[serde(rename = "chainId")]
    chain_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AcrossTransaction {
    to: String,
    data: String,
    #[serde(rename = "chainId")]
    chain_id: Option<u64>,
}

fn get_chain_info(name: &str) -> Option<(u64, &'static str)> {
    CHAINS
        .iter()
        .find(|(n, _, _)| *n == name.to_lowercase())
        .map(|(_, id, addr)| (*id, *addr))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("===========================================");
    println!("  Bridge USDC Test - Across Protocol API");
    println!("===========================================\n");

    // Test parameters
    let from_chain = "base";
    let to_chain = "polygon";
    let amount_usdc = "10"; // 10 USDC
    let amount_raw = (amount_usdc.parse::<f64>().unwrap() * 1_000_000.0) as u64;

    // Use a sample address for testing (vitalik.eth)
    let test_wallet = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";

    let (from_chain_id, from_usdc) = get_chain_info(from_chain)
        .ok_or_else(|| format!("Unknown chain: {}", from_chain))?;
    let (to_chain_id, to_usdc) = get_chain_info(to_chain)
        .ok_or_else(|| format!("Unknown chain: {}", to_chain))?;

    println!("Bridge Request:");
    println!("  From: {} (chain ID: {})", from_chain, from_chain_id);
    println!("  To: {} (chain ID: {})", to_chain, to_chain_id);
    println!("  Amount: {} USDC ({} raw)", amount_usdc, amount_raw);
    println!("  Wallet: {}", test_wallet);
    println!();

    // Build Across API URL
    let url = format!(
        "{}/swap/approval?tradeType=exactInput&amount={}&inputToken={}&originChainId={}&outputToken={}&destinationChainId={}&depositor={}&recipient={}&slippage=0.005",
        ACROSS_API_URL,
        amount_raw,
        from_usdc,
        from_chain_id,
        to_usdc,
        to_chain_id,
        test_wallet,
        test_wallet
    );

    println!("Calling Across API...");
    println!("URL: {}\n", url);

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;
    let status = response.status();

    if !status.is_success() {
        let error_text = response.text().await?;
        println!("API Error ({}): {}", status, error_text);
        return Ok(());
    }

    let response_text = response.text().await?;
    println!("Raw Response:\n{}\n", &response_text[..response_text.len().min(500)]);

    let data: AcrossSwapResponse = serde_json::from_str(&response_text)?;

    println!("===========================================");
    println!("  Across API Response");
    println!("===========================================\n");

    // Expected output
    if let Some(expected) = &data.expected_output_amount {
        let expected_usdc = expected.parse::<u64>().unwrap_or(0) as f64 / 1_000_000.0;
        println!("Expected Output: {:.4} USDC", expected_usdc);
        let fee_usdc = amount_usdc.parse::<f64>().unwrap() - expected_usdc;
        println!("Estimated Fee: {:.4} USDC ({:.2}%)", fee_usdc, (fee_usdc / amount_usdc.parse::<f64>().unwrap()) * 100.0);
    }

    // Fill time
    if let Some(fill_time) = data.expected_fill_time {
        println!("Estimated Fill Time: {} seconds", fill_time);
    }

    // Approval transactions
    println!("\n--- Approval Transaction(s) ---");
    if !data.approval_txns.is_empty() {
        for (i, approval) in data.approval_txns.iter().enumerate() {
            println!("Approval #{}: to={}", i + 1, approval.to);
            println!("  Data: {}...", &approval.data[..approval.data.len().min(66)]);
            println!("  Chain ID: {:?}", approval.chain_id);
        }
        println!("\n[Approval needed - this approves USDC spend]");
    } else {
        println!("[No approval needed - already approved or infinite approval]");
    }

    // Bridge transaction
    println!("\n--- Bridge Transaction ---");
    if let Some(swap_tx) = &data.swap_tx {
        println!("To (Across SpokePool): {}", swap_tx.to);
        println!("Data: {}...", &swap_tx.data[..swap_tx.data.len().min(66)]);
        println!("Chain ID: {:?}", swap_tx.chain_id);
    } else {
        println!("[ERROR: No swap transaction returned]");
    }

    // Show fees breakdown if available
    if let Some(fees) = &data.fees {
        println!("\n--- Fee Breakdown ---");
        println!("{}", serde_json::to_string_pretty(fees)?);
    }

    println!("\n===========================================");
    println!("  How the Bridge Flow Works");
    println!("===========================================\n");

    println!("1. User calls: bridge_usdc from_chain=base to_chain=polygon amount=10");
    println!("2. Tool calls Across API to get quote and transaction data");
    println!("3. If approval needed: Queue approval tx (approve USDC spend)");
    println!("4. Queue bridge tx (deposit to Across SpokePool)");
    println!("5. User reviews with: list_queued_web3_tx");
    println!("6. User broadcasts with: broadcast_web3_tx");
    println!("7. Across relayers fill the order on destination chain (~2 sec)");
    println!("8. User receives USDC on Polygon!");

    println!("\n===========================================");
    println!("  Example Tool Usage");
    println!("===========================================\n");

    println!(r#"
// Step 1: Bridge USDC (queues transactions)
{{
  "tool": "bridge_usdc",
  "params": {{
    "from_chain": "base",
    "to_chain": "polygon",
    "amount": "100"
  }}
}}

// Step 2: Review queued transactions
{{
  "tool": "list_queued_web3_tx",
  "params": {{
    "status": "pending"
  }}
}}

// Step 3: Broadcast approval (if needed)
{{
  "tool": "broadcast_web3_tx",
  "params": {{
    "uuid": "<approval_uuid>"
  }}
}}

// Step 4: Wait for approval confirmation, then broadcast bridge
{{
  "tool": "broadcast_web3_tx",
  "params": {{
    "uuid": "<bridge_uuid>"
  }}
}}
"#);

    Ok(())
}
