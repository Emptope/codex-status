import { describe, expect, it, vi } from 'vitest';
import { shouldDrag } from '../../src/ui/drag';

function gesture(blocked = false, button = 0, isPrimary = true) {
  return {
    button,
    isPrimary,
    target: { closest: vi.fn(() => (blocked ? {} : null)) } as unknown as EventTarget,
  };
}

describe('card dragging', () => {
  it('starts from a primary pointer on a non-interactive surface', () => {
    expect(shouldDrag(gesture())).toBe(true);
  });

  it('leaves controls and secondary pointers interactive', () => {
    expect(shouldDrag(gesture(true))).toBe(false);
    expect(shouldDrag(gesture(false, 2))).toBe(false);
    expect(shouldDrag(gesture(false, 0, false))).toBe(false);
  });
});
