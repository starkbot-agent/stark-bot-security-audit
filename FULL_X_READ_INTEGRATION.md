# X (Twitter) API Read Integration Guide

## Overview

To read tweets/posts from X's API, you need a **paid tier**. The Free tier is write-only (posting) and cannot read data.

## Pricing Tiers for Read Access

| Tier | Price | Read Limit | Notes |
|------|-------|------------|-------|
| **Free** | $0 | **0** (no read access) | Write-only: 1,500 posts/month |
| **Basic** | $200/mo ($175/mo annual) | 10,000 posts/month | Entry-level read access |
| **Pro** | $5,000/mo | Higher limits | For commercial/enterprise use |

> **Note**: Basic tier was $100/mo until October 2024, then raised to $200/mo with additional endpoints.

## Basic Tier ($200/month) - What You Get

- **10,000 tweet reads per month**
- 50,000 tweet posts per month
- **7 days of search history** (recent search only)
- 2 App environments
- Access to both API v2 and Standard v1.1
- Endpoints include `reposts_of_me`, community search

### Key Limitations

- 7-day search window (no historical data beyond 7 days)
- 10K reads depletes fast for any real monitoring use case
- No Full Archive Search (requires Pro or Enterprise)

## Setup Steps

### 1. Create Developer Account

1. Go to [developer.x.com](https://developer.x.com)
2. Sign in with your X account
3. Complete the onboarding wizard

### 2. Create a Project

1. Click **"Create Project"** in Developer Portal
2. Name your project (e.g., "Tweet Reader")
3. Describe your use case
4. You start on Free tier by default

### 3. Upgrade to Basic Tier

1. Go to **"Products"** section in Developer Portal
2. Find **X API v2** card
3. Click **"View Access Levels"**
4. Select **Basic** tier
5. Enter payment information ($200/month)

### 4. Create an App & Get Credentials

1. Inside your project, click **"Create App"**
2. Name your app
3. Go to **"Keys and Tokens"** tab
4. Save these credentials (shown only once):

```
API_KEY=xxxxxxxxxxxxxxxx          # Consumer Key (public)
API_SECRET=xxxxxxxxxxxxxxxx       # Consumer Secret (KEEP SECRET)
BEARER_TOKEN=xxxxxxxxxxxxxxxx     # For app-only auth (read public data)
ACCESS_TOKEN=xxxxxxxxxxxxxxxx     # For user-context auth
ACCESS_TOKEN_SECRET=xxxxxxxx      # For user-context auth
```

## Authentication for Read Access

### Bearer Token (Recommended for Reading Public Data)

Best for reading public tweets without user involvement.

```bash
curl -X GET "https://api.twitter.com/2/tweets/search/recent?query=starknet" \
  -H "Authorization: Bearer YOUR_BEARER_TOKEN"
```

### OAuth 2.0 (For User-Specific Data)

Required if you need to read a user's private data or act on their behalf.

## Key Read Endpoints (API v2)

| Endpoint | Description | Basic Tier |
|----------|-------------|------------|
| `GET /2/tweets/:id` | Get single tweet by ID | Yes |
| `GET /2/tweets` | Get multiple tweets by IDs | Yes |
| `GET /2/tweets/search/recent` | Search tweets (last 7 days) | Yes |
| `GET /2/users/:id/tweets` | Get user's tweets | Yes |
| `GET /2/users/:id/mentions` | Get user's mentions | Yes |
| `GET /2/users/:id/timelines/reverse_chronological` | Home timeline | Yes |
| `GET /2/tweets/search/all` | Full archive search | **No (Pro only)** |

## Example: Search Recent Tweets

```python
import requests

BEARER_TOKEN = "your_bearer_token"

def search_tweets(query, max_results=10):
    url = "https://api.twitter.com/2/tweets/search/recent"
    headers = {"Authorization": f"Bearer {BEARER_TOKEN}"}
    params = {
        "query": query,
        "max_results": max_results,
        "tweet.fields": "created_at,author_id,text,public_metrics"
    }

    response = requests.get(url, headers=headers, params=params)
    return response.json()

# Search for tweets about Starknet
tweets = search_tweets("starknet -is:retweet")
for tweet in tweets.get("data", []):
    print(f"{tweet['created_at']}: {tweet['text'][:100]}...")
```

## Rate Limits (Basic Tier)

| Endpoint | Requests per 15 min |
|----------|---------------------|
| Tweet lookup | 900 |
| Recent search | 450 |
| User tweets | 1,500 |
| User mentions | 450 |

## Cost Optimization Tips

1. **Cache aggressively** - Store tweets locally to avoid re-fetching
2. **Use specific queries** - Narrow searches reduce wasted reads
3. **Batch requests** - Fetch multiple tweet IDs in one call
4. **Monitor usage** - Track your 10K monthly limit in Developer Portal

## Alternative: Pay-Per-Use Pilot (Closed Beta)

X launched a pay-per-use pilot in late 2025:
- Pay only for API requests made
- Credit-based system
- $500 voucher for beta testers
- Currently in closed beta

## Quick Decision

| Need | Solution | Cost |
|------|----------|------|
| Post tweets only | Free tier | $0 |
| Read up to 10K tweets/mo | Basic tier | $200/mo |
| Search beyond 7 days | Pro tier | $5,000/mo |
| High volume reading | Third-party providers | Varies |

## Sources

- [X/Twitter API Pricing Guide](https://getlate.dev/blog/twitter-api-pricing)
- [TechCrunch: X API Price Increase](https://techcrunch.com/2024/10/30/x-makes-its-basic-api-tier-more-costly-launches-annual-subscriptions/)
- [X API Setup Guide](https://elfsight.com/blog/how-to-get-x-twitter-api-key-in-2025/)
- [X Developer Portal](https://developer.x.com)
- [X Pay-Per-Use Announcement](https://devcommunity.x.com/t/announcing-the-x-api-pay-per-use-pricing-pilot/250253)
