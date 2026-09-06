export type View = 'summary' | 'details' | 'sessions' | 'settings';

const widths: Record<View, number> = {
  summary: 300,
  details: 360,
  sessions: 380,
  settings: 400,
};

export function viewWidth(view: View): number {
  return widths[view];
}

export function draggedHeight(height: number, startY: number, currentY: number): number {
  return Math.max(40, Math.round(height + currentY - startY));
}
