---
name: starkhub
description: "Browse, search, install, and submit skills on StarkHub (hub.starkbot.ai) — the decentralized skills directory for StarkBot agents."
version: 1.0.2
author: starkbot
homepage: https://hub.starkbot.ai
metadata: {"clawdbot":{"emoji":"🌐"}}
requires_tools: [web_fetch, manage_skills]
tags: [general, all, skills, hub, discovery, meta, management]
arguments:
  query:
    description: "Search query, skill slug, or tag name"
    required: false
  username:
    description: "Author username (without @) for scoped skill operations"
    required: false
  action:
    description: "What to do: search, trending, featured, browse, tags, view, install, submit"
    required: false
    default: "trending"
---

# StarkHub — Skill Directory

StarkHub (https://hub.starkbot.ai) is the public skills marketplace for StarkBot agents. Use it to discover, install, and publish skills.

**Base URL:** `https://hub.starkbot.ai/api`

All read endpoints are public. Submitting requires authentication.

**Important:** Skills are scoped to authors using the `@username/slug` format (like npm packages). Most skill-specific endpoints require both the author's username and the skill slug.

---

## Discovery

### Search for Skills

```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/search?q={{query}}&limit=20",
  "extract_mode": "raw"
}
```

### Trending Skills (top 20 by installs)

```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/skills/trending",
  "extract_mode": "raw"
}
```

### Featured Skills

```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/skills/featured",
  "extract_mode": "raw"
}
```

### Browse with Sorting and Filters

```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/skills?sort=new&per_page=20&page=1",
  "extract_mode": "raw"
}
```

Sort options: `trending`, `new`, `top`, `name`

Filter by tag:
```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/skills?tag=defi&sort=top",
  "extract_mode": "raw"
}
```

### List All Tags

```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/tags",
  "extract_mode": "raw"
}
```

Skills by tag:
```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/tags/{{query}}",
  "extract_mode": "raw"
}
```

---

## View Skill Details

Get full info for a skill by its `@username/slug`:

```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/skills/@{{username}}/{{query}}",
  "extract_mode": "raw"
}
```

Returns: `name`, `description`, `version`, `content`, `raw_markdown`, `tags`, `requires_tools`, `install_count`, `author`, `x402_cost`.

---

## Install a Skill from StarkHub

**Step 1:** Download the raw skill markdown:

```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/skills/@{{username}}/{{query}}/raw",
  "extract_mode": "raw"
}
```

**Step 2:** Install locally:

```json
{
  "tool": "manage_skills",
  "action": "install",
  "markdown": "<the raw markdown from step 1>"
}
```

If the skill already exists locally, use `"action": "update"` instead.

**Step 3 (optional):** Record the install on StarkHub:

```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/skills/@{{username}}/{{query}}/install",
  "method": "POST",
  "extract_mode": "raw"
}
```

---

## Submit a Skill to StarkHub

Publishing a skill to StarkHub requires authentication and a StarkLicense NFT.

### Step 1: Authenticate (SIWE)

Get a nonce:

```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/auth/nonce",
  "extract_mode": "raw"
}
```

Returns `{"nonce": "..."}`.

Then verify (sign in). The `wallet_address` register contains the agent's address:

```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/auth/verify",
  "method": "POST",
  "body": {
    "message": "Sign in to StarkHub\nNonce: <nonce>\nAddress: <wallet_address>",
    "signature": "0x",
    "address": "<wallet_address>"
  },
  "extract_mode": "raw"
}
```

Returns `{"token": "...", "wallet_address": "...", ...}`.

**Save the `token`** — you need it for the submit request.

### Step 2: Prepare the Skill Markdown

The skill must be a valid SKILL.md with YAML frontmatter:

```markdown
---
name: my-skill
description: "What this skill does"
version: 1.0.0
tags: [category1, category2]
requires_tools: [tool1, tool2]
---

# Skill Title

Instructions for the agent...
```

If submitting an existing local skill, read its markdown with `manage_skills`:

```json
{
  "tool": "manage_skills",
  "action": "get",
  "name": "skill_name"
}
```

The `prompt_template` field contains the body. You will need to reconstruct the full markdown with frontmatter.

### Step 3: Submit

```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/submit",
  "method": "POST",
  "headers": {
    "Authorization": "Bearer <token from step 1>"
  },
  "body": {
    "raw_markdown": "<full skill markdown with frontmatter>"
  },
  "extract_mode": "raw"
}
```

Returns `{"success": true, "slug": "my-skill", "username": "your-username", "id": "...", "status": "pending"}`.

**Note:** Submitted skills start with status `pending` and require admin approval before they appear publicly.

### Requirements

- **StarkLicense NFT**: The authenticated wallet must hold a StarkLicense NFT (ERC-721 on Base: `0xa23a42D266653846e05d8f356a52298844537472`)
- **Rate limit**: Maximum 5 submissions per 24 hours
- **Required fields**: `name`, `description`, `version` in the frontmatter

### Step 4: Update an Existing Skill

To update a skill you already submitted:

```json
{
  "tool": "web_fetch",
  "url": "https://hub.starkbot.ai/api/skills/@{{username}}/{{query}}",
  "method": "PUT",
  "headers": {
    "Authorization": "Bearer <token>"
  },
  "body": {
    "raw_markdown": "<updated full skill markdown>"
  },
  "extract_mode": "raw"
}
```

Only the original author can update their skill.

---

## Workflow Guide

### "Find me a skill for X"

1. Search: `GET /api/search?q=X`
2. Present results with name, description, install count, author username, and tags
3. If user picks one, install it using `@username/slug`

### "What's popular on StarkHub?"

1. Fetch trending: `GET /api/skills/trending`
2. Summarize top results

### "Install @username/slug from StarkHub"

1. Download raw: `GET /api/skills/@{username}/{slug}/raw`
2. Install locally via `manage_skills` → `install`
3. Record install: `POST /api/skills/@{username}/{slug}/install`

### "Publish my skill to StarkHub"

1. Authenticate via SIWE (nonce → verify → token)
2. Read the local skill markdown (via `manage_skills` get or `read_file`)
3. Submit via `POST /api/submit` with auth header
4. Confirm pending status to user

### "What categories exist?"

1. Fetch tags: `GET /api/tags`
2. List names and skill counts

---

## Response Formats

### Skill Summary (search/list)

```json
{
  "slug": "my-skill",
  "name": "My Skill",
  "description": "What it does",
  "version": "1.0.0",
  "author_name": "builder",
  "author_address": "0x...",
  "author_username": "builder",
  "install_count": 42,
  "featured": false,
  "x402_cost": "0",
  "status": "active",
  "tags": ["defi", "trading"],
  "created_at": "2025-01-01T00:00:00Z"
}
```

Use `author_username` + `slug` to construct the scoped URL: `/api/skills/@{author_username}/{slug}`

### Skill Detail (/skills/@{username}/{slug})

```json
{
  "slug": "my-skill",
  "name": "My Skill",
  "description": "What it does",
  "version": "1.0.0",
  "author": {
    "wallet_address": "0x...",
    "username": "builder",
    "display_name": "builder",
    "verified": true
  },
  "raw_markdown": "---\nname: ...\n---\n...",
  "install_count": 42,
  "tags": ["defi"],
  "requires_tools": ["web_fetch"],
  "x402_cost": "0"
}
```

---

## Paid Skills (x402)

Skills with `x402_cost` > `"0"` cost STARKBOT tokens to install. The install endpoint may return **402 Payment Required** with x402 payment instructions.

---

## Tips

- **Scoped URLs** use the `@username/slug` format — always include the author's username
- **`author_username`** from search/list results tells you the username to use in skill URLs
- **`extract_mode: "raw"`** is required — the API returns JSON, not HTML
- After installing, the skill is immediately available — verify with `manage_skills` list
- If a skill name conflicts locally, use `manage_skills` update instead of install
- Submitted skills need admin approval before they go live
