---
name: Skills
---

Skills are reusable modules that extend the agent with specialized behaviors, instructions, and tool access.

## What's a Skill?

A markdown file with YAML frontmatter that defines:

- **Behavior** — Instructions the agent follows
- **Arguments** — Parameters the skill accepts
- **Tools** — Which tools the skill can use

## Basic Format

```markdown
---
name: weather
description: Get weather information for a location
arguments:
  - name: location
    description: City or location name
    required: true
tools:
  - web_search
  - web_fetch
---

# Weather Skill

When asked about weather:

1. Search for current conditions at the location
2. Fetch detailed forecast if requested
3. Summarize clearly with temperature, conditions, and alerts
```

---

## Frontmatter Reference

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Unique identifier (lowercase, hyphens) |
| `description` | string | When to use this skill |
| `arguments` | array | Parameters the skill accepts |
| `tools` | array | Tools the skill can access |
| `requires_tools` | array | Tools that MUST be available (force-included) |
| `version` | string | Skill version (optional) |
| `tags` | array | Categorization tags for subtype filtering |

### Skill Tags

Tags determine which agent subtypes can use a skill:

| Tags | Available To |
|------|-------------|
| `general`, `all` | All subtypes |
| `crypto`, `defi`, `transfer`, `swap`, `finance`, `wallet`, `token` | Finance |
| `development`, `git`, `testing`, `debugging`, `review`, `code`, `github` | CodeEngineer |
| `social`, `marketing`, `messaging`, `moltx`, `scheduling` | Secretary |

### Arguments

```yaml
arguments:
  - name: location
    description: Target city or region
    required: true
  - name: units
    description: Temperature units
    required: false
    default: celsius
```

---

## Creating Skills

### Method 1: Single File

Upload a `.md` file through **Skills** in the dashboard.

### Method 2: ZIP Archive

For skills with multiple files:

```
my-skill.zip
├── skill.md          # Main definition (required)
├── templates/        # Optional templates
│   └── report.md
└── data/             # Optional data files
    └── config.json
```

---

## Examples

### GitHub PR Skill

```markdown
---
name: github-pr
description: Create and review GitHub pull requests
arguments:
  - name: action
    description: create, review, or merge
    required: true
  - name: repo
    description: Repository (owner/repo)
    required: true
tools:
  - exec
  - read_file
---

# GitHub PR Skill

## Create PR
1. Check current branch: `git branch --show-current`
2. Verify changes are committed
3. Push and create PR: `gh pr create`

## Review PR
1. Get PR details: `gh pr view`
2. Review changed files
3. Summarize what changed and why

## Merge PR
1. Check CI status: `gh pr checks`
2. Merge: `gh pr merge --squash`
```

### Research Skill

```markdown
---
name: research
description: Conduct web research on a topic
arguments:
  - name: topic
    required: true
  - name: depth
    description: quick, standard, or thorough
    default: standard
tools:
  - web_search
  - web_fetch
---

# Research Skill

## Quick
- Single search, top 3 results summarized

## Standard
- Multiple queries from different angles
- Cross-reference information
- Cite sources

## Thorough
- Comprehensive search coverage
- Deep dive into authoritative sources
- Fact verification
- Structured report with citations
```

### Daily Summary Skill

```markdown
---
name: daily-summary
description: Generate daily activity summary
tags:
  - general
tools:
  - agent_send
---

# Daily Summary Skill

Generate a summary covering:

1. Messages processed today
2. Tools used and outcomes
3. Scheduled jobs that ran
4. Notable events or errors

Format as bullet points. If a channel is specified, send the summary there.
```

### Token Transfer Skill (Finance)

```markdown
---
name: transfer
description: Transfer tokens to an address
tags:
  - crypto
  - transfer
  - finance
requires_tools:
  - token_lookup
  - web3_function_call
  - web3_tx
---

# Token Transfer Skill

## Steps
1. Look up token address with `token_lookup`
2. Check user's balance with `web3_function_call` (preset: erc20_balance)
3. Build transfer transaction with `web3_tx`
4. Transaction will be queued for user approval
5. Once approved, broadcast with `broadcast_web3_tx`

## Important
- Always confirm recipient address with user
- Show token balance before transfer
- Wait for user approval before broadcasting
```

### Swap Skill (Finance)

```markdown
---
name: swap
description: Swap tokens using DEX
tags:
  - crypto
  - defi
  - swap
requires_tools:
  - token_lookup
  - x402_rpc
  - web3_tx
---

# Swap Skill

## Steps
1. Look up both token addresses
2. Get current price quote
3. Build swap transaction
4. Queue for user approval
5. Broadcast on approval

## Networks
- Base: Use 0x Settler or Uniswap
- Ethereum: Use Uniswap or 1inch
```

---

## Using Skills

### In Chat

Ask naturally:

> "Use the research skill to find information about WebAssembly"

Or be explicit:

> "Run the github-pr skill with action=review and repo=myorg/myrepo"

### In Cron Jobs

```json
{
  "name": "Morning Briefing",
  "schedule": "0 8 * * *",
  "message": "Use the daily-summary skill and send to Discord"
}
```

### Slash Command

In Agent Chat:

```
/skills
```

Lists all available skills.

---

## Managing Skills

| Action | How |
|--------|-----|
| **View** | Go to Skills page |
| **Upload** | Click Upload, select .md or .zip |
| **Enable/Disable** | Toggle switch |
| **Update** | Upload with same name |
| **Delete** | Click delete button |

---

## Best Practices

1. **Clear names** — `github-pr` not `pr-skill-v2`
2. **Detailed descriptions** — Help the AI know when to use it
3. **Minimal tools** — Only request what's needed
4. **Step-by-step** — Guide the AI through the process
5. **Handle failures** — Include instructions for errors
