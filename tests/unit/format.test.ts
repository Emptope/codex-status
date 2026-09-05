import { describe, expect, it } from 'vitest';
import { countdown, duration, number, percent, time } from '../../src/state/format';

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
});
