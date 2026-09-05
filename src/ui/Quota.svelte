<script lang="ts">
  import type { QuotaWindow } from '../types/status';
  import { countdown, duration, percent, time } from '../state/format';
  let {
    window: quota,
    now,
    details = false,
  }: { window: QuotaWindow; now: number; details?: boolean } = $props();
</script>

<div class="quota-row" class:stale={quota.remaining.quality === 'stale'}>
  <div class="quota-label">
    <span>{duration(quota.minutes)}</span><strong>{percent(quota.remaining.value)}</strong><time
      title={time(quota.resetsAt)}>{countdown(quota.resetsAt, now)}</time
    >
  </div>
  {#if quota.remaining.value !== null}
    <meter
      min="0"
      max="100"
      low="20"
      optimum="100"
      value={quota.remaining.value}
      aria-label={`${duration(quota.minutes)} quota remaining`}
    ></meter>
  {/if}
  {#if details}<div class="quota-detail">
      <span>{quota.remaining.quality === 'stale' ? 'Expired' : 'Resets at'}</span><span
        >{time(quota.resetsAt)}</span
      >
    </div>{/if}
</div>
