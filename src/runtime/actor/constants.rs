// Shared small timing constants used across actor-style loops
// Keep values conservative to balance responsiveness and CPU usage

/// Idle tick interval for select! loops (milliseconds)
pub const ACTOR_IDLE_TICK_MS: u64 = 50;

/// Command poll timeout when checking control channel (milliseconds)
pub const ACTOR_CMD_POLL_TIMEOUT_MS: u64 = 10;
