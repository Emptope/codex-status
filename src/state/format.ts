import type { Activity } from '../types/status';

export const activity: Record<Activity, string> = {
  unknown: 'Unknown',
  idle: 'Idle',
  running: 'Running',
  waitingApproval: 'Approval needed',
  waitingInput: 'Input needed',
  completed: 'Completed',
  failed: 'Failed',
  interrupted: 'Interrupted',
};
export const connection: Record<string, string> = {
  connecting: 'Connecting',
  connected: 'Connected',
  offline: 'Offline',
  unavailable: 'CLI not found',
  unsupported: 'Unsupported account',
  signedOut: 'Signed out',
  apiKey: 'API key',
  externalProvider: 'External provider',
  chatgpt: 'ChatGPT',
  unknown: 'Unknown',
};
export function connectionLabel(value: string, provider: string | null): string {
  return value === 'externalProvider' && provider ? provider : connection[value] || 'Unknown';
}
export const error: Record<string, string> = {
  'auth-required': 'Sign in required',
  'query-failed': 'Could not refresh',
  'query-timeout': 'Refresh timed out',
  'record-limit': 'Record too large',
  'record-unreadable': 'Record unavailable',
  'response-invalid': 'Invalid source response',
  'response-too-large': 'Source response too large',
  'settings-invalid': 'Settings reset',
  'settings-recovered': 'Settings recovered',
  'source-exited': 'Source stopped',
  'source-limit': 'Too many records',
  'source-missing': 'Data source not found',
  'source-read-failed': 'Source unavailable',
  'source-start-failed': 'Could not start CLI',
  'source-unreadable': 'Data source unavailable',
  'source-write-failed': 'Source unavailable',
  'unsupported-method': 'CLI method unavailable',
};
export function unit(value: number, name: string): string {
  return `${value} ${name}${value === 1 ? '' : 's'}`;
}
export function duration(minutes: number | null): string {
  if (minutes === null) return 'Quota';
  if (minutes % 1440 === 0) return unit(minutes / 1440, 'day');
  if (minutes % 60 === 0) return unit(minutes / 60, 'hour');
  return `${minutes} min`;
}
export function countdown(at: number | null, now: number): string {
  if (at === null) return 'Unknown';
  if (at <= now) return 'Expired';
  const minutes = Math.ceil((at - now) / 60000);
  if (minutes < 60) return `${minutes}m`;
  if (minutes < 1440) return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
  return `${Math.floor(minutes / 1440)}d ${Math.floor((minutes % 1440) / 60)}h`;
}
export function number(value: number | null | undefined): string {
  return value === null || value === undefined
    ? 'Unknown'
    : new Intl.NumberFormat('en-US').format(value);
}
export function time(value: number | null | undefined): string {
  return value === null || value === undefined
    ? 'Unknown'
    : new Date(value).toLocaleString('en-US');
}
export function percent(value: number | null | undefined): string {
  return value === null || value === undefined ? 'Unavailable' : `${Math.round(value)}%`;
}
