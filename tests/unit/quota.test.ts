import { describe, expect, it } from 'vitest';
import { quotaLevel } from '../../src/state/quota';

describe('quota levels', () => {
  it('uses non-overlapping boundaries for each remaining quota range', () => {
    expect(quotaLevel(null)).toBe(null);
    expect(quotaLevel(0)).toBe('low');
    expect(quotaLevel(9.99)).toBe('low');
    expect(quotaLevel(10)).toBe('medium');
    expect(quotaLevel(49.99)).toBe('medium');
    expect(quotaLevel(50)).toBe('high');
    expect(quotaLevel(100)).toBe('high');
  });
});
