import { describe, expect, it } from 'vitest';
import { draggedHeight, viewWidth } from '../../src/ui/layout';

describe('window layout', () => {
  it('gives dense views only the width their content needs', () => {
    expect(viewWidth('summary')).toBe(300);
    expect(viewWidth('details')).toBe(360);
    expect(viewWidth('sessions')).toBe(380);
    expect(viewWidth('settings')).toBe(400);
  });

  it('resizes from the bottom without changing the minimum height', () => {
    expect(draggedHeight(480, 700, 760)).toBe(540);
    expect(draggedHeight(480, 700, 600)).toBe(380);
    expect(draggedHeight(80, 700, 100)).toBe(40);
  });
});
