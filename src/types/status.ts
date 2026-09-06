export type Quality = 'fresh' | 'stale' | 'unavailable' | 'unsupported';
export interface Field<T> {
  value: T | null;
  source: string;
  observedAt: number | null;
  quality: Quality;
}
export type Activity =
  | 'unknown'
  | 'idle'
  | 'running'
  | 'waitingApproval'
  | 'waitingInput'
  | 'completed'
  | 'failed'
  | 'interrupted';
export interface Usage {
  input: number | null;
  cachedInput: number | null;
  output: number | null;
  total: number | null;
}
export interface Session {
  id: string;
  path: string;
  project: string;
  activity: Field<Activity>;
  model: Field<string>;
  effort: Field<string>;
  usage: Field<Usage>;
  lastUsage: Field<Usage>;
  contextLimit: Field<number>;
  contextUsed: Field<number>;
  latestAt: number;
  turnStartedAt: number | null;
  durationMs: number | null;
}
export interface QuotaWindow {
  remaining: Field<number>;
  minutes: number | null;
  resetsAt: number | null;
}
export interface Quota {
  id: string;
  name: string;
  windows: QuotaWindow[];
  creditBalance: string | null;
  unlimitedCredits: boolean | null;
}
export interface Snapshot {
  revision: number;
  sessions: Session[];
  quotas: Quota[];
  connection: string;
  account: string;
  provider: string | null;
  updatedAt: number | null;
  error: string | null;
  localError: string | null;
  refreshing: boolean;
}
export type QuotaSound = 'off' | 'alert' | 'battery';
export interface Settings {
  roots: string[];
  executable: string;
  theme: string;
  fontSize: number;
  alwaysOnTop: boolean;
  collapsed: boolean;
  autoFollow: boolean;
  pinnedSession: string | null;
  selectedBucket: string | null;
  notifications: boolean;
  completionSound: boolean;
  quotaSound: QuotaSound;
  muted: boolean;
  lowQuota: number;
  position: [number, number] | null;
}
export const empty: Snapshot = {
  revision: 0,
  sessions: [],
  quotas: [],
  connection: 'connecting',
  account: 'unknown',
  provider: null,
  updatedAt: null,
  error: null,
  localError: null,
  refreshing: false,
};
export const defaults: Settings = {
  roots: [],
  executable: 'codex',
  theme: 'system',
  fontSize: 13,
  alwaysOnTop: true,
  collapsed: false,
  autoFollow: true,
  pinnedSession: null,
  selectedBucket: null,
  notifications: true,
  completionSound: true,
  quotaSound: 'alert',
  muted: false,
  lowQuota: 10,
  position: null,
};
