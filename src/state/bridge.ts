import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { Settings, Snapshot } from '../types/status';

export const native = isTauri();
export async function command<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  if (native) return invoke<T>(name, args);
  const response = await fetch(`/api/${name}`, {
    method: name === 'snapshot' || name === 'preferences' ? 'GET' : 'POST',
    headers: { 'Content-Type': 'application/json' },
    ...(name === 'snapshot' || name === 'preferences' ? {} : { body: JSON.stringify(args) }),
  });
  if (!response.ok) throw new Error('Connection unavailable');
  return response.status === 204 ? (undefined as T) : response.json();
}
export async function subscribe(
  update: (snapshot: Snapshot) => void,
  settings: () => void,
): Promise<() => void> {
  if (native) {
    const stop = await listen<Snapshot>('status', (event) => update(event.payload));
    const menu = await listen('open-settings', settings);
    return () => {
      stop();
      menu();
    };
  }
  const timer = window.setInterval(() => {
    command<Snapshot>('snapshot')
      .then(update)
      .catch(() => {});
  }, 1000);
  return () => clearInterval(timer);
}
export async function drag() {
  if (native) await getCurrentWindow().startDragging();
}
export async function resizeHeight(height: number) {
  if (!native) return false;
  await command('resize', { width: innerWidth, height });
  return true;
}
export async function fit(width: number, height: number) {
  if (native) await command('resize', { width, height });
}
export async function save(settings: Settings) {
  await command('save_preferences', { settings });
}
