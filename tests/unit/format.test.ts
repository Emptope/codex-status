import { describe, expect, it } from 'vitest';
import {
  connectionLabel,
  countdown,
  duration,
  nextCountdownUpdate,
  number,
  percent,
  time,
} from '../../src/state/format';

describe('status formatting', () => {
  it('keeps missing and zero values distinct', () => {
    expect(percent(null)).toBe('Unavailable');
    expect(percent(0)).toBe('0%');
    expect(number(undefined)).toBe('Unknown');
    expect(number(0)).toBe('0');
  });

  it('derives labels and countdowns from source values', () => {
    const now = Date.UTC(2026, 8, 5, 0, 0, 0);
    expect(duration(45)).toBe('45 min');
    expect(duration(300)).toBe('5 hours');
    expect(duration(10080)).toBe('7 days');
    expect(countdown(now, now)).toBe('Expired');
    expect(countdown(now + 61 * 60_000, now)).toBe('1h 1m');
    expect(time(null)).toBe('Unknown');
    expect(time(0)).not.toBe('Unknown');
  });

  it('schedules countdown work only when the visible minute changes', () => {
    const now = Date.UTC(2026, 8, 5, 0, 0, 0);
    expect(nextCountdownUpdate([null, now - 1], now)).toBeNull();
    expect(nextCountdownUpdate([now + 30_000], now)).toBe(30_000);
    expect(nextCountdownUpdate([now + 60_001], now)).toBe(1);
    expect(nextCountdownUpdate([now + 150_000, now + 20_000], now)).toBe(20_000);
  });

  it('shows a verified external provider name', () => {
    expect(connectionLabel('externalProvider', 'Work gateway')).toBe('Work gateway');
    expect(connectionLabel('externalProvider', null)).toBe('External provider');
    expect(connectionLabel('connected', 'Work gateway')).toBe('Connected');
  });
});
