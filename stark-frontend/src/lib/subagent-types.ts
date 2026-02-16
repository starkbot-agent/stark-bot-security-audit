// Matches Rust SubAgentStatus enum with snake_case serialization
// From stark-backend/src/ai/multi_agent/types.rs
export enum SubagentStatus {
  Pending = 'pending',
  Running = 'running',
  Completed = 'completed',
  Failed = 'failed',
  TimedOut = 'timed_out',
  Cancelled = 'cancelled',
}

export interface Subagent {
  id: string;
  label: string;
  task: string;
  status: SubagentStatus;
  started_at: string;
}

// Helper to check if a subagent is active (running or pending)
export function isSubagentActive(subagent: Subagent): boolean {
  return subagent.status === SubagentStatus.Running || subagent.status === SubagentStatus.Pending;
}
