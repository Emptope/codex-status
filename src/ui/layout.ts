export type View = 'summary' | 'details' | 'sessions' | 'settings';

export const shadowInsets = Object.freeze({ horizontal: 20, top: 14, bottom: 26 });

const widths: Record<View, number> = {
  summary: 300,
  details: 360,
  sessions: 380,
  settings: 400,
};

export function viewWidth(view: View): number {
  return widths[view];
}

export function framedWidth(width: number): number {
  return width + shadowInsets.horizontal * 2;
}

export function framedHeight(height: number): number {
  return height + shadowInsets.top + shadowInsets.bottom;
}

export function draggedHeight(
  height: number,
  startY: number,
  currentY: number,
  minimum = 40,
): number {
  return Math.max(minimum, Math.round(height + currentY - startY));
}
