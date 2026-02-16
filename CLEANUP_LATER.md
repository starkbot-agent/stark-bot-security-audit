# Cleanup Later

Dead code and backwards-compat shims that can be removed once all deployments have migrated.

## bot_token column on external_channels (Feb 2026)

**Context:** bot_token was moved from a column on `external_channels` to per-channel settings (`discord_bot_token`, `telegram_bot_token`, `slack_bot_token`, `slack_app_token`). The column is kept for backwards compatibility with old backups.

**What to remove after ~1 year (Feb 2027):**

1. **DB schema** (`db/sqlite.rs`): Remove `bot_token TEXT NOT NULL DEFAULT ''` column from `external_channels` CREATE TABLE. Write a migration to drop it.
2. **Channel model** (`models/channel.rs`):
   - Remove `bot_token: String` from `Channel` struct
   - Remove `app_token: Option<String>` from `Channel` struct
   - Remove `bot_token` and `app_token` from `ChannelResponse`
   - Remove `bot_token` and `app_token` from `UpdateChannelRequest`
3. **DB queries** (`db/tables/channels.rs`):
   - Remove `bot_token` and `app_token` params from `create_channel()`, `create_channel_with_safe_mode()`, `create_safe_mode_channel()`
   - Remove `bot_token` and `app_token` from `update_channel()`
   - Remove these columns from all SELECT/INSERT/UPDATE queries on `external_channels`
4. **Channel manager** (`channels/mod.rs`): The settings-loading block in `start_channel()` can drop the "fall back to column" behavior — just load from settings unconditionally.
5. **Backup restore migration** (`controllers/api_keys.rs` and `main.rs`): Remove the "Migrate legacy bot_token column → channel setting" blocks that copy `channel.bot_token` into settings on restore.
6. **Backup struct** (`backup/mod.rs`): Make `bot_token` optional in `ChannelEntry` (or remove it entirely if no old backups need support).
7. **Safe mode rate limiter** (`channels/safe_mode_rate_limiter.rs`): `ChannelCreationRequest` still has `bot_token: String` field — remove it and update `create_safe_mode_channel()` signature.
8. **Frontend** (`lib/api.ts`): Remove `bot_token` and `app_token` from `ChannelInfo` interface if the backend stops sending them.
