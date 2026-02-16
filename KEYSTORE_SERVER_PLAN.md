# Keystore API Server Plan

**URL:** `https://keystore.defirelay.com`

## Overview

A secure blob storage service for encrypted API key backups. Uses SIWE (Sign-In With Ethereum) authentication to verify wallet ownership before allowing read/write access. The server never sees decrypted data - it just stores and retrieves encrypted strings keyed by wallet address.

The stark-backend handles all encryption/decryption using ECIES with the burner wallet private key. This service authenticates and stores the encrypted blobs.

## Tech Stack
- **Runtime:** Node.js/Express or Rust/Actix-web
- **Database:** PostgreSQL (or SQLite for simplicity)
- **Hosting:** Railway, Fly.io, or similar
- **Auth:** SIWE (Sign-In With Ethereum)

---

## Authentication Flow

```
┌─────────────────┐         ┌─────────────────┐
│  Starkbot       │         │  Keystore       │
│  Backend        │         │  Server         │
└────────┬────────┘         └────────┬────────┘
         │                           │
         │  POST /api/authorize      │
         │  { address: "0x..." }     │
         │ ─────────────────────────>│
         │                           │
         │  { challenge: "..." }     │
         │ <─────────────────────────│
         │                           │
         │  [Sign challenge with     │
         │   burner wallet pkey]     │
         │                           │
         │  POST /api/authorize/verify
         │  { address, signature }   │
         │ ─────────────────────────>│
         │                           │
         │  { token: "session_xxx" } │
         │ <─────────────────────────│
         │                           │
         │  [Store token in memory]  │
         │                           │
         │  POST /api/store_keys     │
         │  Authorization: Bearer xxx│
         │ ─────────────────────────>│
         │                           │
         │  { success: true }        │
         │ <─────────────────────────│
         │                           │
```

---

## Database Schema

```sql
CREATE TABLE backups (
    id SERIAL PRIMARY KEY,
    wallet_id VARCHAR(42) NOT NULL UNIQUE,  -- Ethereum address (0x...)
    encrypted_data TEXT NOT NULL,            -- Hex-encoded ECIES encrypted blob
    key_count INTEGER NOT NULL DEFAULT 0,    -- Informational only
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_backups_wallet_id ON backups(wallet_id);

-- Session tokens (short-lived, can also use in-memory store like Redis)
CREATE TABLE sessions (
    id SERIAL PRIMARY KEY,
    token VARCHAR(64) NOT NULL UNIQUE,
    wallet_id VARCHAR(42) NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_sessions_token ON sessions(token);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);

-- Pending challenges (short-lived)
CREATE TABLE challenges (
    id SERIAL PRIMARY KEY,
    wallet_id VARCHAR(42) NOT NULL,
    nonce VARCHAR(32) NOT NULL,
    message TEXT NOT NULL,              -- Full SIWE message to sign
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_challenges_wallet ON challenges(wallet_id);
```

---

## API Endpoints

### 1. `POST /api/authorize` - Request SIWE Challenge

**Request:**
```json
{
  "address": "0x742d35Cc6634C0532925a3b844Bc454e4438f44e"
}
```

**Response (200):**
```json
{
  "success": true,
  "message": "keystore.defirelay.com wants you to sign in with your Ethereum account:\n0x742d35Cc6634C0532925a3b844Bc454e4438f44e\n\nSign in to Keystore API\n\nURI: https://keystore.defirelay.com\nVersion: 1\nChain ID: 1\nNonce: abc123xyz\nIssued At: 2026-02-01T12:00:00Z\nExpiration Time: 2026-02-01T12:05:00Z",
  "nonce": "abc123xyz"
}
```

**Logic:**
- Validate address format
- Generate random nonce
- Create SIWE message with 5-minute expiry
- Store challenge in DB (or memory)
- Return message for signing

---

### 2. `POST /api/authorize/verify` - Verify Signature & Get Token

**Request:**
```json
{
  "address": "0x742d35Cc6634C0532925a3b844Bc454e4438f44e",
  "signature": "0x..."
}
```

**Response (200):**
```json
{
  "success": true,
  "token": "ks_a1b2c3d4e5f6...",
  "expires_at": "2026-02-01T13:00:00Z"
}
```

**Response (401):**
```json
{
  "success": false,
  "error": "Invalid signature"
}
```

**Logic:**
- Look up pending challenge for address
- Verify signature matches SIWE message
- Verify challenge not expired
- Generate session token (1 hour expiry)
- Delete used challenge
- Return token

---

### 3. `POST /api/store_keys` - Store Encrypted Backup

**Headers:**
```
Authorization: Bearer ks_a1b2c3d4e5f6...
```

**Request:**
```json
{
  "encrypted_data": "04a3f2b1c4d5e6f7...",
  "key_count": 5
}
```

**Response (200):**
```json
{
  "success": true,
  "message": "Backup stored",
  "key_count": 5,
  "updated_at": "2026-02-01T12:00:00Z"
}
```

**Response (401):**
```json
{
  "success": false,
  "error": "Invalid or expired token"
}
```

**Logic:**
- Validate session token
- Extract wallet_id from session
- Upsert encrypted data for that wallet

---

### 4. `POST /api/get_keys` - Retrieve Encrypted Backup

**Headers:**
```
Authorization: Bearer ks_a1b2c3d4e5f6...
```

**Response (200):**
```json
{
  "success": true,
  "encrypted_data": "04a3f2b1c4d5e6f7...",
  "key_count": 5,
  "updated_at": "2026-02-01T12:00:00Z"
}
```

**Response (404):**
```json
{
  "success": false,
  "error": "No backup found for this wallet"
}
```

**Logic:**
- Validate session token
- Extract wallet_id from session
- Return encrypted data for that wallet

---

### 5. `GET /api/health` - Health Check

**Response (200):**
```json
{
  "status": "ok"
}
```

---

## Security

1. **SIWE Authentication** - Proves wallet ownership before any data access
2. **Session tokens** - 1 hour expiry, stored securely
3. **Challenge expiry** - 5 minutes to complete auth
4. **Rate limiting** - 10 auth attempts/min, 10 writes/min, 30 reads/min per IP
5. **Size limit** - Max 1MB for `encrypted_data`
6. **HTTPS only**
7. **CORS** - Allow from your frontend domains

---

## Starkbot Backend Integration

The starkbot backend should:

1. **Cache session tokens in memory** with expiry tracking
2. **Auto-authenticate** when token is missing or expired
3. **Retry once** if a request fails with 401 (token may have expired)

```rust
// Pseudocode for keystore client
struct KeystoreClient {
    session_token: Option<String>,
    token_expires_at: Option<DateTime>,
}

impl KeystoreClient {
    async fn ensure_authenticated(&mut self, private_key: &str) -> Result<()> {
        if self.is_token_valid() {
            return Ok(());
        }

        // Step 1: Get challenge
        let challenge = self.request_challenge(address).await?;

        // Step 2: Sign with wallet
        let signature = sign_siwe_message(private_key, &challenge.message)?;

        // Step 3: Verify and get token
        let session = self.verify_signature(address, signature).await?;

        self.session_token = Some(session.token);
        self.token_expires_at = Some(session.expires_at);
        Ok(())
    }

    async fn store_keys(&mut self, private_key: &str, data: &str) -> Result<()> {
        self.ensure_authenticated(private_key).await?;
        // POST /api/store_keys with Bearer token
    }

    async fn get_keys(&mut self, private_key: &str) -> Result<String> {
        self.ensure_authenticated(private_key).await?;
        // POST /api/get_keys with Bearer token
    }
}
```

---

## Example Implementation (Node.js)

```javascript
const express = require('express');
const { Pool } = require('pg');
const cors = require('cors');
const rateLimit = require('express-rate-limit');
const crypto = require('crypto');
const { SiweMessage } = require('siwe');

const app = express();
const pool = new Pool({ connectionString: process.env.DATABASE_URL });

app.use(cors({ origin: ['https://stark.defirelay.com', 'http://localhost:5173'] }));
app.use(express.json({ limit: '1mb' }));

// Rate limiting
const authLimiter = rateLimit({ windowMs: 60000, max: 10 });
const writeLimiter = rateLimit({ windowMs: 60000, max: 10 });
const readLimiter = rateLimit({ windowMs: 60000, max: 30 });

// Helper: Generate random token
const generateToken = () => 'ks_' + crypto.randomBytes(32).toString('hex');
const generateNonce = () => crypto.randomBytes(16).toString('hex');

// Helper: Validate session token
async function validateSession(req) {
  const auth = req.headers.authorization;
  if (!auth?.startsWith('Bearer ')) return null;

  const token = auth.slice(7);
  const result = await pool.query(
    'SELECT wallet_id FROM sessions WHERE token = $1 AND expires_at > NOW()',
    [token]
  );
  return result.rows[0]?.wallet_id || null;
}

// Health check
app.get('/api/health', (req, res) => {
  res.json({ status: 'ok' });
});

// Step 1: Request SIWE challenge
app.post('/api/authorize', authLimiter, async (req, res) => {
  const { address } = req.body;

  if (!address?.match(/^0x[a-fA-F0-9]{40}$/i)) {
    return res.status(400).json({ success: false, error: 'Invalid address format' });
  }

  const nonce = generateNonce();
  const now = new Date();
  const expiresAt = new Date(now.getTime() + 5 * 60 * 1000); // 5 min

  const message = new SiweMessage({
    domain: 'keystore.defirelay.com',
    address: address,
    statement: 'Sign in to Keystore API',
    uri: 'https://keystore.defirelay.com',
    version: '1',
    chainId: 1,
    nonce: nonce,
    issuedAt: now.toISOString(),
    expirationTime: expiresAt.toISOString(),
  }).prepareMessage();

  // Store challenge
  await pool.query(
    'INSERT INTO challenges (wallet_id, nonce, message, expires_at) VALUES (LOWER($1), $2, $3, $4)',
    [address, nonce, message, expiresAt]
  );

  res.json({ success: true, message, nonce });
});

// Step 2: Verify signature and issue token
app.post('/api/authorize/verify', authLimiter, async (req, res) => {
  const { address, signature } = req.body;

  if (!address?.match(/^0x[a-fA-F0-9]{40}$/i)) {
    return res.status(400).json({ success: false, error: 'Invalid address format' });
  }

  // Get pending challenge
  const challengeResult = await pool.query(
    'SELECT message FROM challenges WHERE wallet_id = LOWER($1) AND expires_at > NOW() ORDER BY created_at DESC LIMIT 1',
    [address]
  );

  if (challengeResult.rows.length === 0) {
    return res.status(401).json({ success: false, error: 'No pending challenge or expired' });
  }

  try {
    const siweMessage = new SiweMessage(challengeResult.rows[0].message);
    await siweMessage.verify({ signature });

    // Delete used challenge
    await pool.query('DELETE FROM challenges WHERE wallet_id = LOWER($1)', [address]);

    // Create session
    const token = generateToken();
    const expiresAt = new Date(Date.now() + 60 * 60 * 1000); // 1 hour

    await pool.query(
      'INSERT INTO sessions (token, wallet_id, expires_at) VALUES ($1, LOWER($2), $3)',
      [token, address, expiresAt]
    );

    res.json({ success: true, token, expires_at: expiresAt.toISOString() });
  } catch (err) {
    console.error('Signature verification failed:', err);
    res.status(401).json({ success: false, error: 'Invalid signature' });
  }
});

// Store encrypted keys
app.post('/api/store_keys', writeLimiter, async (req, res) => {
  const walletId = await validateSession(req);
  if (!walletId) {
    return res.status(401).json({ success: false, error: 'Invalid or expired token' });
  }

  const { encrypted_data, key_count } = req.body;

  if (!encrypted_data || !/^[a-fA-F0-9]+$/.test(encrypted_data)) {
    return res.status(400).json({ success: false, error: 'Invalid encrypted_data format' });
  }

  try {
    const result = await pool.query(`
      INSERT INTO backups (wallet_id, encrypted_data, key_count, updated_at)
      VALUES ($1, $2, $3, NOW())
      ON CONFLICT (wallet_id)
      DO UPDATE SET encrypted_data = $2, key_count = $3, updated_at = NOW()
      RETURNING updated_at, key_count
    `, [walletId, encrypted_data, key_count || 0]);

    res.json({
      success: true,
      message: 'Backup stored',
      key_count: result.rows[0].key_count,
      updated_at: result.rows[0].updated_at
    });
  } catch (err) {
    console.error('Database error:', err);
    res.status(500).json({ success: false, error: 'Database error' });
  }
});

// Retrieve encrypted keys
app.post('/api/get_keys', readLimiter, async (req, res) => {
  const walletId = await validateSession(req);
  if (!walletId) {
    return res.status(401).json({ success: false, error: 'Invalid or expired token' });
  }

  try {
    const result = await pool.query(
      'SELECT encrypted_data, key_count, updated_at FROM backups WHERE wallet_id = $1',
      [walletId]
    );

    if (result.rows.length === 0) {
      return res.status(404).json({ success: false, error: 'No backup found for this wallet' });
    }

    res.json({
      success: true,
      encrypted_data: result.rows[0].encrypted_data,
      key_count: result.rows[0].key_count,
      updated_at: result.rows[0].updated_at
    });
  } catch (err) {
    console.error('Database error:', err);
    res.status(500).json({ success: false, error: 'Database error' });
  }
});

// Cleanup expired sessions/challenges (run periodically)
async function cleanup() {
  await pool.query('DELETE FROM sessions WHERE expires_at < NOW()');
  await pool.query('DELETE FROM challenges WHERE expires_at < NOW()');
}
setInterval(cleanup, 60000); // Every minute

const PORT = process.env.PORT || 3000;
app.listen(PORT, () => console.log(`Keystore API running on port ${PORT}`));
```

---

## Deployment Checklist

1. Create PostgreSQL database
2. Run schema migration (including sessions and challenges tables)
3. Install `siwe` package: `npm install siwe`
4. Set environment variables:
   - `DATABASE_URL`
   - `PORT`
5. Deploy to Railway/Fly.io
6. Configure DNS for `keystore.defirelay.com`
7. Verify HTTPS is working
8. Test auth flow:
   ```bash
   # Step 1: Get challenge
   curl -X POST https://keystore.defirelay.com/api/authorize \
     -H "Content-Type: application/json" \
     -d '{"address":"0x742d35Cc6634C0532925a3b844Bc454e4438f44e"}'

   # Step 2: Sign message (done in backend with ethers.js)
   # Step 3: Verify and get token
   curl -X POST https://keystore.defirelay.com/api/authorize/verify \
     -H "Content-Type: application/json" \
     -d '{"address":"0x...","signature":"0x..."}'

   # Step 4: Store keys
   curl -X POST https://keystore.defirelay.com/api/store_keys \
     -H "Content-Type: application/json" \
     -H "Authorization: Bearer ks_abc123..." \
     -d '{"encrypted_data":"abc123","key_count":1}'

   # Step 5: Get keys
   curl -X POST https://keystore.defirelay.com/api/get_keys \
     -H "Authorization: Bearer ks_abc123..."
   ```
