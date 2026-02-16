---
name: gog
description: Google Workspace CLI — Gmail, Calendar, Drive, Contacts, Sheets, Docs.
version: 1.3.0
requires_binaries: [gog]
tags: [google, gmail, calendar, drive, sheets, docs, productivity]
homepage: https://gogcli.sh
requires_api_keys:
  GOG_ACCOUNT:
    description: Google account email
    secret: false
  GOG_CLIENT_CREDENTIALS:
    description: OAuth client_secret.json content
  GOG_KEYRING_PASSWORD:
    description: Password for headless token storage
---

# gog — Google Workspace CLI

Use `gog` via bash to interact with Google Workspace. `GOG_ACCOUNT` and `GOG_KEYRING_PASSWORD` are injected automatically from API keys. Use `--json --no-input` for machine-readable output.

## Required API keys
- `GOG_ACCOUNT` — Google account email (auto-injected as env var)
- `GOG_KEYRING_PASSWORD` — password for non-interactive token storage on headless servers (auto-injected as env var)
- `GOG_CLIENT_CREDENTIALS` — OAuth client_secret.json content (for initial credential setup)

## First-time setup (headless server)
1. Write credentials and register them with gog:
```
echo "$GOG_CLIENT_CREDENTIALS" > /tmp/gog_creds.json && gog auth credentials /tmp/gog_creds.json && rm /tmp/gog_creds.json
```
2. Start the remote auth flow — this prints a URL for the user to open in their browser:
```
gog auth add $GOG_ACCOUNT --services gmail,calendar,drive,contacts,sheets,docs --remote --step 1
```
3. After the user authorizes and provides the callback URL, complete the flow:
```
gog auth add $GOG_ACCOUNT --remote --step 2 --auth-url '<callback-url-from-user>'
```

## Gmail
```
gog gmail search 'newer_than:7d' --max 10
gog gmail send --to user@example.com --subject "Subject" --body "Body"
```

## Calendar
```
gog calendar events <calendarId> --from <iso> --to <iso>
```

## Drive
```
gog drive search "query" --max 10
```

## Contacts
```
gog contacts list --max 20
```

## Sheets
```
gog sheets get <sheetId> "Tab!A1:D10" --json
gog sheets update <sheetId> "Tab!A1:B2" --values-json '[["A","B"],["1","2"]]' --input USER_ENTERED
gog sheets append <sheetId> "Tab!A:C" --values-json '[["x","y","z"]]' --insert INSERT_ROWS
gog sheets clear <sheetId> "Tab!A2:Z"
gog sheets metadata <sheetId> --json
```

## Docs
```
gog docs cat <docId>
gog docs export <docId> --format txt --out /tmp/doc.txt
```

## Rules
- Always confirm before sending mail or creating/modifying events.
- Prefer `--values-json` for sheets data.
- Use `gog <service> --help` to discover subcommands.
