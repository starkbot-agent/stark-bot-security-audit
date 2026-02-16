# x402 Payment Protocol for AI Agents

This document describes how StarkBot integrates with x402-enabled AI endpoints for pay-per-use API access.

## Overview

The x402 protocol enables HTTP-native payments where:
1. Client makes a request to an AI endpoint
2. Server returns HTTP 402 Payment Required with payment details
3. Client signs an EIP-3009 authorization for USDC transfer
4. Client retries with `X-PAYMENT` header containing the signed authorization
5. Server verifies payment and processes the request

## Supported Endpoints

The following endpoints use x402 payment protocol:
- `https://llama.defirelay.com/api/v1/chat/completions` - Llama models
- `https://kimi.defirelay.com/api/v1/chat/completions` - Kimi models

**Note:** The endpoint path includes `/api/v1/...` not just `/v1/...`

## Configuration

### Environment Variable

```bash
BURNER_WALLET_BOT_PRIVATE_KEY=0x...your_private_key...
```

The burner wallet private key is used to sign EIP-3009 authorizations. This wallet must have USDC on Base mainnet (chain ID 8453) to pay for API calls.

### Agent Settings

In the StarkBot UI, select one of the defirelay endpoints from the dropdown:
- `llama.defirelay.com` - Uses Llama models
- `kimi.defirelay.com` - Uses Kimi models

When these endpoints are selected, x402 payment handling is automatically enabled.

## Technical Details

### Payment Flow

```
┌─────────┐          ┌─────────────┐          ┌──────────┐
│ StarkBot│          │ AI Endpoint │          │   USDC   │
└────┬────┘          └──────┬──────┘          └────┬─────┘
     │                      │                      │
     │  POST /chat/completions                     │
     │─────────────────────>│                      │
     │                      │                      │
     │  402 Payment Required                       │
     │  (payment details)   │                      │
     │<─────────────────────│                      │
     │                      │                      │
     │  Sign EIP-3009       │                      │
     │  Authorization       │                      │
     │                      │                      │
     │  POST /chat/completions                     │
     │  X-PAYMENT: {...}    │                      │
     │─────────────────────>│                      │
     │                      │                      │
     │                      │  TransferWithAuth    │
     │                      │─────────────────────>│
     │                      │                      │
     │  200 OK              │                      │
     │  (AI response)       │                      │
     │<─────────────────────│                      │
     │                      │                      │
```

### EIP-3009 Authorization

The payment is made using EIP-3009 `TransferWithAuthorization` which allows gasless USDC transfers. The signed authorization includes:

- `from`: Burner wallet address
- `to`: Payment receiver (from 402 response)
- `value`: Payment amount in USDC (6 decimals)
- `validAfter`: Timestamp when authorization becomes valid
- `validBefore`: Expiration timestamp
- `nonce`: Unique nonce from 402 response

### Network

All payments are made on **Base mainnet** (chain ID 8453) using the USDC contract at:
```
0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913
```

## Module Structure

```
stark-backend/src/x402/
├── mod.rs      # Module exports
├── types.rs    # Data structures (PaymentRequired, PaymentPayload, etc.)
├── signer.rs   # EIP-3009 signing with LocalWallet
└── client.rs   # HTTP client with automatic 402 handling
```

### Key Components

**X402Client** - HTTP client that wraps reqwest and handles 402 responses:
```rust
let client = X402Client::new(private_key)?;
let response = client.post_with_payment(url, &request).await?;
```

**X402Signer** - Signs EIP-3009 authorizations:
```rust
let signer = X402Signer::new(private_key)?;
let authorization = signer.sign_authorization(&requirements).await?;
```

**is_x402_endpoint** - Detects x402-enabled endpoints:
```rust
if is_x402_endpoint(&url) {
    // Enable x402 payment handling
}
```

## Funding the Burner Wallet

1. Generate a new Ethereum wallet or use an existing one
2. Export the private key (with 0x prefix)
3. Add to `.env` as `BURNER_WALLET_BOT_PRIVATE_KEY`
4. Send USDC to the wallet address on Base mainnet
5. Monitor balance and top up as needed

The wallet address is logged on startup:
```
[AI] x402 enabled for endpoint https://llama.defirelay.com/v1/chat/completions with wallet 0x...
```

## Cost Considerations

- Each AI request costs a small amount of USDC (typically fractions of a cent)
- Costs vary by model and token usage
- Monitor your burner wallet balance regularly
- Set up alerts for low balance if running in production

## Troubleshooting

### "x402 request failed"
- Check that `BURNER_WALLET_BOT_PRIVATE_KEY` is set correctly
- Verify the wallet has sufficient USDC on Base mainnet
- Check network connectivity to the endpoint

### "Failed to create x402 client"
- Ensure the private key is valid (64 hex chars with 0x prefix)
- Check for any special characters in the environment variable

### Payment not processing
- Verify USDC balance on Base mainnet (not other networks)
- Check that the wallet hasn't been flagged or blacklisted
- Review server logs for detailed error messages
