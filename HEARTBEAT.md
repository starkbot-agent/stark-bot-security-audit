# Heartbeat System

The heartbeat system provides periodic automated check-ins that allow the AI agent to review pending tasks, notifications, and scheduled items without manual user intervention.

## Overview

Heartbeats are scheduled periodic messages dispatched to the AI agent. Unlike cron jobs which execute specific commands, heartbeats trigger a general review prompt that allows the agent to proactively check on things that need attention.

## Architecture

```
┌─────────────────┐     ┌──────────────┐     ┌─────────────────────┐
│   Scheduler     │────►│   Database   │────►│  MessageDispatcher  │
│  (60s polling)  │     │ (SQLite)     │     │                     │
└─────────────────┘     └──────────────┘     └─────────────────────┘
        │                                              │
        │ GatewayEvent                                 │ NormalizedMessage
        ▼                                              ▼
┌─────────────────┐                          ┌─────────────────────┐
│ EventBroadcaster│                          │    AI Orchestrator  │
│ (WebSocket)     │                          │                     │
└─────────────────┘                          └─────────────────────┘
```

## Configuration

### Database Schema

Heartbeat configurations are stored in the `heartbeat_configs` table:

| Field | Type | Description |
|-------|------|-------------|
| `id` | INTEGER | Primary key |
| `channel_id` | INTEGER | Optional - ties heartbeat to a specific channel |
| `interval_minutes` | INTEGER | How often to run (default: 30) |
| `target` | TEXT | Session target (default: 'last') |
| `active_hours_start` | TEXT | Start of active window (HH:MM format) |
| `active_hours_end` | TEXT | End of active window (HH:MM format) |
| `active_days` | TEXT | Comma-separated days (mon,tue,wed,thu,fri,sat,sun) |
| `enabled` | BOOLEAN | Whether heartbeat is active |
| `last_beat_at` | TEXT | Timestamp of last execution |
| `next_beat_at` | TEXT | Timestamp of next scheduled execution |

### REST API

All endpoints require Bearer token authentication.

#### Global Heartbeat Config

```
GET  /api/heartbeat/config         # Get global heartbeat configuration
PUT  /api/heartbeat/config         # Update global heartbeat configuration
```

#### Channel-Specific Config

```
GET  /api/heartbeat/config/:id     # Get heartbeat config for channel
PUT  /api/heartbeat/config/:id     # Update heartbeat config for channel
```

#### Update Request Body

```json
{
  "interval_minutes": 30,
  "active_hours_start": "09:00",
  "active_hours_end": "17:00",
  "active_days": "mon,tue,wed,thu,fri",
  "enabled": true
}
```

## Scheduler Configuration

The scheduler is configured in `SchedulerConfig`:

```rust
pub struct SchedulerConfig {
    pub cron_enabled: bool,        // Enable cron job processing
    pub heartbeat_enabled: bool,   // Enable heartbeat processing (default: false)
    pub poll_interval_secs: u64,   // Poll interval (default: 60 seconds)
    pub max_concurrent_jobs: usize // Max concurrent executions (default: 5)
}
```

**Note:** Heartbeats are disabled by default. To enable, set `heartbeat_enabled: true` in the scheduler configuration.

## Execution Flow

1. **Polling**: Scheduler checks every 60 seconds for due heartbeats
2. **Query**: `list_due_heartbeat_configs()` finds enabled configs where `next_beat_at <= now`
3. **Active Hours Check**: Verifies current time is within configured active window
4. **Race Prevention**: Updates `next_beat_at` BEFORE execution to prevent duplicate runs
5. **Dispatch**: Creates a `NormalizedMessage` with:
   - Text: `"[HEARTBEAT] Periodic check - review any pending tasks, notifications, or scheduled items."`
   - Channel type: `heartbeat`
   - Session mode: `isolated` (separate from main conversation)
6. **Broadcast**: Sends `heartbeat_started` and `heartbeat_completed` WebSocket events
7. **Update**: Records `last_beat_at` after completion

## Active Hours

Heartbeats can be restricted to specific time windows:

### Time Format
- Use 24-hour format: `HH:MM` (e.g., `09:00`, `17:30`)
- Both `active_hours_start` and `active_hours_end` must be set for time filtering

### Day Format
- Comma-separated lowercase day abbreviations
- Valid values: `mon`, `tue`, `wed`, `thu`, `fri`, `sat`, `sun`
- Example: `"mon,tue,wed,thu,fri"` for weekdays only

### Examples

**Business hours only:**
```json
{
  "active_hours_start": "09:00",
  "active_hours_end": "17:00",
  "active_days": "mon,tue,wed,thu,fri"
}
```

**Weekend mornings:**
```json
{
  "active_hours_start": "08:00",
  "active_hours_end": "12:00",
  "active_days": "sat,sun"
}
```

## WebSocket Events

The heartbeat system broadcasts events for real-time monitoring:

### heartbeat_started
```json
{
  "type": "custom",
  "event": "heartbeat_started",
  "data": {
    "config_id": 1,
    "channel_id": null
  }
}
```

### heartbeat_completed
```json
{
  "type": "custom",
  "event": "heartbeat_completed",
  "data": {
    "config_id": 1,
    "channel_id": null,
    "success": true
  }
}
```

## Response Suppression

If the AI agent responds with `HEARTBEAT_OK` anywhere in its response, the output is suppressed from normal delivery. This allows the agent to acknowledge the heartbeat without creating noise when nothing needs attention.

## Best Practices

1. **Start with longer intervals**: Begin with 60+ minute intervals and decrease only if needed
2. **Use active hours**: Avoid heartbeats during off-hours to reduce unnecessary processing
3. **Channel-specific configs**: Use channel-specific heartbeats for high-priority channels
4. **Monitor execution**: Watch `heartbeat_completed` events to ensure heartbeats are running
5. **Check last_beat_at**: If `last_beat_at` is stale, the heartbeat may have stopped

## Troubleshooting

### Heartbeats not running
1. Check `heartbeat_enabled` is `true` in scheduler config
2. Verify the heartbeat config has `enabled: true`
3. Check active hours/days constraints
4. Look for errors in server logs

### Duplicate heartbeats
The system updates `next_beat_at` before execution to prevent this. If duplicates occur:
1. Check if poll interval is shorter than typical execution time
2. Review database for stuck `next_beat_at` values

### Missing events
1. Ensure WebSocket connection is active
2. Check EventBroadcaster is receiving events
3. Verify no authentication issues on the gateway

## Files

| File | Purpose |
|------|---------|
| `src/scheduler/runner.rs` | Main scheduler with heartbeat execution |
| `src/db/tables/heartbeat.rs` | Database operations for heartbeat configs |
| `src/controllers/cron.rs` | REST API endpoints |
| `src/models/cron_job.rs` | HeartbeatConfig data model |
