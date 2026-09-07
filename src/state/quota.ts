type QuotaLevel = 'low' | 'medium' | 'high';

export function quotaLevel(remaining: number | null): QuotaLevel | null {
  if (remaining === null) return null;
  if (remaining <= 10) return 'low';
  if (remaining <= 50) return 'medium';
  return 'high';
}
