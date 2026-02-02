---
name: StarkBot Documentation
---

StarkBot is a self-hosted AI agent platform that connects to messaging platforms, executes tools, remembers context, and integrates with Web3.

## What is StarkBot?

A Rust-powered backend with a React dashboard that turns AI models into autonomous agents:

- **Multi-platform** — Telegram, Slack, Discord, and web chat
- **Task Planning** — Automatically breaks down complex requests into discrete tasks
- **Agent Subtypes** — Specialized toolboxes for Finance, Development, and Social tasks
- **Tool execution** — Web search, file ops, shell commands, blockchain transactions
- **Persistent memory** — Cross-session context with automatic consolidation
- **Scheduled tasks** — Cron jobs and heartbeat automation
- **Web3 native** — Wallet auth, x402 payments, transaction queue, on-chain identity (EIP-8004)

## Core Capabilities

| Feature | Description |
|---------|-------------|
| **Task Planner** | Intelligently breaks down requests into actionable steps |
| **Agent Subtypes** | Finance (DeFi), CodeEngineer (dev), Secretary (social) toolboxes |
| **Channels** | Connect multiple platforms simultaneously |
| **AI Providers** | Claude, OpenAI, Llama with streaming and tool calling |
| **50+ Tools** | Web, filesystem, exec, messaging, finance, and development |
| **Skills** | Extend capabilities with custom markdown modules |
| **Memory** | Facts, preferences, tasks, and daily logs |
| **Transaction Queue** | Review and approve Web3 transactions before broadcast |
| **x402 Payments** | Crypto micropayments for pay-per-use AI services |
| **EIP-8004** | On-chain agent identity and reputation system |
| **Real-time** | WebSocket events for tool progress and transactions |

## Architecture at a Glance

```
┌─────────────────────────────────────────────────────┐
│            Telegram · Slack · Discord · Web          │
└─────────────────────────┬───────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────┐
│              Message Dispatcher (Rust)              │
│  normalize → task planner → AI → tools → memory     │
└─────────────────────────┬───────────────────────────┘
          ┌───────────────┼───────────────┐
          ▼               ▼               ▼
     ┌─────────┐    ┌──────────┐    ┌──────────┐
     │   AI    │    │  Tools   │    │  SQLite  │
     │ Claude  │    │ Registry │    │ Database │
     │ OpenAI  │    │  50+     │    │          │
     │ Llama   │    │ 9 Groups │    │ Tx Queue │
     └─────────┘    └──────────┘    └──────────┘
```

## Agent Subtypes

When you send a message, the agent first selects a specialized toolbox:

| Subtype | Use Case | Key Tools |
|---------|----------|-----------|
| **Finance** | Crypto, swaps, DeFi, balances | x402_rpc, web3_tx, token_lookup |
| **CodeEngineer** | Code editing, git, testing | grep, glob, edit_file, exec, git |
| **Secretary** | Social media, messaging | agent_send, moltx tools |

## Quick Links

| Section | What You'll Learn |
|---------|-------------------|
| [Getting Started](/docs/getting-started) | Run StarkBot in 5 minutes |
| [Architecture](/docs/architecture) | System design deep dive |
| [Tools](/docs/tools) | Built-in capabilities |
| [Skills](/docs/skills) | Custom extensions |
| [Channels](/docs/channels) | Platform integrations |
| [Scheduling](/docs/scheduling) | Automation and cron |
| [Memories](/docs/memories) | Long-term context |
| [Configuration](/docs/configuration) | Environment and settings |
| [API Reference](/docs/api) | REST and WebSocket APIs |

## Tech Stack

| Layer | Technology |
|-------|------------|
| Backend | Rust · Actix-web · Tokio |
| Frontend | React · TypeScript · Vite |
| Database | SQLite |
| WebSocket | tokio-tungstenite |
| Styling | Tailwind CSS |
| AI | Anthropic Claude · OpenAI · Llama (Kimi) |
| Auth | Sign In With Ethereum (SIWE) |
| Web3 | ethers-rs · x402 protocol · Multi-chain support |
| Identity | EIP-8004 on-chain agent registry |
