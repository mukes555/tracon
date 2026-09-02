export type SessionSummary = {
  session_id: string;
  agent: string;
  cwd: string | null;
  started_at: string;
  last_at: string;
  event_count: number;
  command_count: number;
  flagged_count: number;
  hook_tool_count: number;
  tail_tool_count: number;
  first_prompt: string | null;
};

export type AgentEvent = {
  id?: number;
  agent: string;
  session_id: string;
  ts: string;
  kind: string;
  source: string;
  cwd: string | null;
  tool_name: string | null;
  summary: string | null;
  flag: string | null;
  payload: unknown;
};

export type Stats = {
  session_count: number;
  event_count: number;
  command_count: number;
  package_count: number;
  flagged_count: number;
  acked_count: number;
  last_event_at: string | null;
  sessions_today: number;
  commands_today: number;
  packages_today: number;
};

export type ChangeToken = {
  max_id: number;
  open_flags: number;
  acked_flags: number;
};

export type ThreadMessage = {
  role: string;
  ts: string;
  text: string;
};

export type DayCount = {
  day: string;
  events: number;
  flagged: number;
};

export type ThemeSetting = "system" | "dark" | "light";

export type CaptureCount = {
  agent: string;
  source: string;
  count: number;
  last_at: string | null;
};

export type CaptureStatus = {
  claude_dir_found: boolean;
  codex_dir_found: boolean;
  cursor_found: boolean;
  counts: CaptureCount[];
};

export type View = "overview" | "live" | "timeline" | "packages" | "flagged" | "settings";

export type KindFilter = "all" | "commands" | "files" | "packages" | "prompts" | "flagged";

export type LiveSession = {
  session_id: string;
  agent: string;
  cwd: string | null;
  last_ts: string;
  event_count: number;
  flagged_count: number;
  subagent_count: number;
  subagents: string[];
  last_prompt: string | null;
  last_action: string | null;
};
