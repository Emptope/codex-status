import { describe, expect, it } from 'vitest';
import {
  draggedHeight,
  framedHeight,
  framedWidth,
  shadowInsets,
  viewWidth,
} from '../../src/ui/layout';

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
    expect(draggedHeight(120, 700, 100, framedHeight(40))).toBe(80);
  });

  it('reserves transparent space around native cards for the custom shadow', () => {
    expect(shadowInsets).toEqual({ horizontal: 20, top: 14, bottom: 26 });
    expect(framedWidth(240)).toBe(280);
    expect(framedHeight(40)).toBe(80);
  });
});
