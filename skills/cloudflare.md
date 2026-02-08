---
name: cloudflare
description: "Manage Cloudflare infrastructure — Workers, DNS records, Pages deployments, and KV storage."
version: 1.0.0
author: starkbot
homepage: https://cloudflare.com
metadata: {"requires_auth": true, "clawdbot":{"emoji":"☁️"}}
requires_tools: [web_fetch, api_keys_check]
tags: [development, devops, cloudflare, infrastructure, workers, dns, pages, kv, deployment]
---

# Cloudflare Integration

Manage Cloudflare infrastructure via the REST API. Deploy Workers, manage DNS records, monitor Pages deployments, and use KV storage.

## Authentication

**First, check if CLOUDFLARE_API_TOKEN is configured:**

```tool:api_keys_check
key_name: CLOUDFLARE_API_TOKEN
```

If not configured, ask the user to create an API token at https://dash.cloudflare.com/profile/api-tokens and add it in Settings > API Keys.

**Then, get the account ID:**

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

Save the `result[0].id` as the account ID for subsequent calls.

---

## How to Use This Skill

All Cloudflare API calls use the `web_fetch` tool:

- **Base URL**: `https://api.cloudflare.com/client/v4`
- **Headers**: `{"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}`
- **extract_mode**: `"raw"` (returns JSON)

The `$CLOUDFLARE_API_TOKEN` placeholder is automatically expanded from the stored API key.

---

## Workers

### List Workers

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/workers/scripts
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

### Get Worker Script Content

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/workers/scripts/SCRIPT_NAME
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN"}
extract_mode: raw
```

### Get Worker Settings

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/workers/scripts/SCRIPT_NAME/settings
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

### Delete Worker

**IMPORTANT: Confirm with user before deleting.**

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/workers/scripts/SCRIPT_NAME
method: DELETE
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

---

## DNS

### List Zones

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/zones
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

### List DNS Records

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/zones/ZONE_ID/dns_records
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

Optional query params: `?type=A&name=example.com&per_page=100`

### Create DNS Record

**IMPORTANT: Confirm with user before creating records.**

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/zones/ZONE_ID/dns_records
method: POST
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
body: {"type": "A", "name": "subdomain.example.com", "content": "198.51.100.4", "ttl": 1, "proxied": true}
extract_mode: raw
```

Supported types: A, AAAA, CNAME, MX, TXT, NS, SRV. Use `ttl: 1` for automatic.

### Update DNS Record

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/zones/ZONE_ID/dns_records/RECORD_ID
method: PATCH
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
body: {"content": "198.51.100.5"}
extract_mode: raw
```

### Delete DNS Record

**IMPORTANT: Confirm with user before deleting.**

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/zones/ZONE_ID/dns_records/RECORD_ID
method: DELETE
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

---

## Pages

### List Projects

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/pages/projects
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

### Get Project Details

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/pages/projects/PROJECT_NAME
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

### List Deployments

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/pages/projects/PROJECT_NAME/deployments
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

Optional: `?env=production` or `?env=preview`

### Get Deployment Status

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/pages/projects/PROJECT_NAME/deployments/DEPLOYMENT_ID
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

### Trigger Deployment

**IMPORTANT: Confirm with user before triggering.**

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/pages/projects/PROJECT_NAME/deployments
method: POST
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
body: {"branch": "main"}
extract_mode: raw
```

### Retry Failed Deployment

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/pages/projects/PROJECT_NAME/deployments/DEPLOYMENT_ID/retry
method: POST
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

### Rollback Deployment

**IMPORTANT: Confirm with user. Can only rollback to successful production builds.**

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/pages/projects/PROJECT_NAME/deployments/DEPLOYMENT_ID/rollback
method: POST
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

---

## KV Storage

### List Namespaces

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/storage/kv/namespaces
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

### Create Namespace

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/storage/kv/namespaces
method: POST
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
body: {"title": "my-namespace"}
extract_mode: raw
```

### List Keys

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/storage/kv/namespaces/NAMESPACE_ID/keys
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

Optional: `?prefix=myprefix&limit=100`

### Read Value

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/storage/kv/namespaces/NAMESPACE_ID/values/KEY_NAME
method: GET
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN"}
extract_mode: raw
```

Note: Returns the raw value, not a JSON envelope.

### Write Value

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/storage/kv/namespaces/NAMESPACE_ID/values/KEY_NAME
method: PUT
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "text/plain"}
body: "your value here"
extract_mode: raw
```

Optional query params: `?expiration_ttl=3600` (seconds, minimum 60)

### Delete Value

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/storage/kv/namespaces/NAMESPACE_ID/values/KEY_NAME
method: DELETE
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

### Delete Namespace

**IMPORTANT: Confirm with user. This deletes ALL keys in the namespace.**

```tool:web_fetch
url: https://api.cloudflare.com/client/v4/accounts/ACCOUNT_ID/storage/kv/namespaces/NAMESPACE_ID
method: DELETE
headers: {"Authorization": "Bearer $CLOUDFLARE_API_TOKEN", "Content-Type": "application/json"}
extract_mode: raw
```

---

## Error Handling

| Error | Cause | Solution |
|-------|-------|----------|
| 401 / Authentication error | Token invalid or expired | Regenerate token at https://dash.cloudflare.com/profile/api-tokens |
| 403 / Forbidden | Token lacks required permission | Check token scopes (e.g., DNS Write, Workers Scripts Write) |
| 404 / Not found | Invalid ID or resource doesn't exist | List resources first to get valid IDs |
| 429 / Rate limited | Too many requests | Wait and retry |
| `success: false` | API error | Check `errors` array in response for details |

---

## Typical Workflow

1. **Verify auth** — list accounts to confirm token works
2. **Discover resources** — list zones, workers, pages projects, KV namespaces
3. **Inspect** — get details on specific resources
4. **Take action** — create, update, deploy (confirm with user first)
5. **Verify** — check deployment status or confirm changes

---

## Best Practices

1. **Always verify auth first** before running other queries
2. **List before acting** — get IDs from list queries, don't guess
3. **Confirm mutations** — always ask the user before creating, updating, or deleting resources
4. **Use proxied mode** for DNS records when possible (Cloudflare protection)
5. **Be careful with KV deletes** — namespace deletion is irreversible
6. **Check token permissions** if you get 403 errors — tokens are scoped
