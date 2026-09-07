<script lang="ts">
  import type { Session } from '../types/status';
  import { activity, number, time } from '../state/format';
  let { session }: { session: Session | undefined } = $props();
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
      <dd>{number(session.contextUsed.value)}</dd>
    </dl>
  {:else}<p class="empty">No session</p>{/if}
</section>
