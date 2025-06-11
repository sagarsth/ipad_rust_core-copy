# Strategic Goal Sync Tracking - Implementation Plan

## Current Status: 🚧 **Waiting for Sync Server**

The database schema and basic infrastructure is ready, but actual sync tracking requires a working sync server.

## What's Already Implemented ✅

### Database Schema
- `last_synced_at` - Timestamp of last successful sync
- `last_sync_attempt_at` - Timestamp of last sync attempt (success or failure)
- `sync_status` - Current sync status (pending/synced/failed/conflict)
- `sync_error_message` - Error details if sync failed
- `sync_version` - Version number for conflict resolution

### Repository Methods (Placeholder)
- `update_sync_status()` - Updates sync timestamp and status
- `find_pending_sync_records()` - Gets records needing sync
- `mark_sync_attempted()` - Marks sync attempt timestamp
- `get_sync_stats()` - Gets sync statistics

### Response DTOs
- `StrategicGoalResponse.last_synced_at` field is ready
- Shows null until first successful sync

## What Needs Implementation When Server Ready 🔄

### 1. Sync Service Integration
```rust
// In sync service after successful upload:
strategic_goal_repo.update_sync_status(
    goal_id,
    Some(Utc::now()), // last_synced_at
    RecordSyncStatus::Synced,
    None, // no error
    Some(new_version),
    None // no transaction
).await?;
```

### 2. Error Handling
```rust
// After failed sync:
strategic_goal_repo.update_sync_status(
    goal_id,
    None, // no successful sync
    RecordSyncStatus::Failed,
    Some("Network timeout"), // error message
    None, // don't increment version on failure
    None
).await?;
```

### 3. Conflict Resolution
```rust
// When server reports conflict:
strategic_goal_repo.update_sync_status(
    goal_id,
    None,
    RecordSyncStatus::Conflict,
    Some("Server version newer"),
    None,
    None
).await?;
```

## Integration Points

### Sync Service Flow
1. `find_pending_sync_records()` - Get goals needing sync
2. `mark_sync_attempted()` - Mark attempt before trying
3. Upload to server
4. On success: `update_sync_status()` with Synced
5. On failure: `update_sync_status()` with Failed + error

### UI Integration
- Show sync status indicators
- Display last sync time
- Show error messages for failed syncs
- Allow manual retry for failed items

## Current Behavior
- All records start with `sync_status = 'pending'`
- `last_synced_at` is null (shows as "Never synced" in UI)
- Repository methods work but don't automatically update sync status
- Need to call sync methods manually when server is ready

## Migration Note
Run this migration when server integration begins:
```sql
UPDATE strategic_goals 
SET sync_status = 'pending' 
WHERE sync_status IS NULL AND deleted_at IS NULL;
``` 