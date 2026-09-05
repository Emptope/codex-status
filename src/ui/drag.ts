const controls =
  'a, button, input, label, select, textarea, [contenteditable="true"], [data-no-drag]';

type Gesture = Pick<PointerEvent, 'button' | 'isPrimary' | 'target'>;
type Target = EventTarget & { closest?: (selector: string) => unknown };

export function shouldDrag(event: Gesture) {
  const target = event.target as Target | null;
  const blocked = typeof target?.closest === 'function' && target.closest(controls);
  return event.button === 0 && event.isPrimary && !blocked;
}
