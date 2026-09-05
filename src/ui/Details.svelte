<script lang="ts">
  import type { Session, Snapshot } from '../types/status';
  import { activity, connection, error, number, time } from '../state/format';
  import Quota from './Quota.svelte';
  let {
    session,
    snapshot,
    now,
  }: { session: Session | undefined; snapshot: Snapshot; now: number } = $props();
</script>

<section aria-label="Session details">
  <h2>Session</h2>
  {#if session}
    <dl>
      <dt>Project</dt>
      <dd class="path">{session.path || 'Unknown'}</dd>
      <dt>Status</dt>
      <dd>{activity[session.activity.value || 'unknown']}</dd>
      <dt>Model</dt>
      <dd>{session.model.value || 'Unknown'}</dd>
      <dt>Reasoning effort</dt>
      <dd>{session.effort.value || 'Unknown'}</dd>
      <dt>Started</dt>
      <dd>{time(session.turnStartedAt)}</dd>
      {#if session.durationMs !== null}<dt>Turn duration</dt>
        <dd>{Math.round(session.durationMs / 1000)} seconds</dd>{/if}
      <dt>Latest event</dt>
      <dd>{time(session.latestAt)}</dd>
    </dl>
    <h2>Token usage</h2>
    <table>
      <thead><tr><th>Type</th><th>Latest</th><th>Session total</th></tr></thead><tbody>
        {#each [['input', 'Input'], ['cachedInput', 'Cached input'], ['output', 'Output'], ['total', 'Total']] as [key, label]}
          <tr
            ><th>{label}</th><td
              >{number(
                session.lastUsage.value?.[key as keyof NonNullable<Session['usage']['value']>],
              )}</td
            ><td
              >{number(
                session.usage.value?.[key as keyof NonNullable<Session['usage']['value']>],
              )}</td
            ></tr
          >
        {/each}
      </tbody>
    </table>
    <h2>Context</h2>
    <dl>
      <dt>Limit</dt>
      <dd>{number(session.contextLimit.value)}</dd>
      <dt>Current usage</dt>
      <dd>
        {session.contextUsed.quality === 'unsupported'
          ? 'Unsupported'
          : number(session.contextUsed.value)}
      </dd>
    </dl>
  {:else}<p class="empty">No session</p>{/if}
  <h2>Account quota</h2>
  {#each snapshot.quotas as bucket (bucket.id)}
    <h3>{bucket.name}</h3>
    {#each bucket.windows as window}<Quota {window} {now} details />{/each}
    {#if bucket.creditBalance !== null}<dl>
        <dt>Credits</dt>
        <dd>{bucket.creditBalance}</dd>
      </dl>{/if}
    {#if bucket.unlimitedCredits}<p>Unlimited credits</p>{/if}
  {:else}<p class="empty">{connection[snapshot.connection] || 'Quota unavailable'}</p>{/each}
  <h2>Connection</h2>
  <dl>
    <dt>Source</dt>
    <dd>Local CLI</dd>
    <dt>Version</dt>
    <dd>{snapshot.version || 'Unknown'}</dd>
    <dt>Connection</dt>
    <dd>{connection[snapshot.connection] || snapshot.connection}</dd>
    <dt>Authentication</dt>
    <dd>{connection[snapshot.account] || snapshot.account}</dd>
    <dt>Last successful update</dt>
    <dd>{time(snapshot.updatedAt)}</dd>
  </dl>
  {#if snapshot.error || snapshot.localError}<p class="error" role="status">
      {error[snapshot.error || snapshot.localError || ''] || 'Source unavailable'}
    </p>{/if}
</section>
