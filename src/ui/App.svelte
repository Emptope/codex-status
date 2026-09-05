<script lang="ts">
  import { onMount } from 'svelte';
  import {
    Activity,
    ChevronDown,
    ChevronUp,
    CircleCheck,
    CircleHelp,
    Clock,
    Pin,
    PinOff,
    RefreshCw,
    Settings as SettingsIcon,
    X,
    Users,
    BellOff,
  } from '@lucide/svelte';
  import { empty, defaults, type Snapshot, type Settings } from '../types/status';
  import { activity, connectionLabel, percent } from '../state/format';
  import { command, drag, fit, native, resizeHeight, save, subscribe } from '../state/bridge';
  import Quota from './Quota.svelte';
  import Details from './Details.svelte';
  import Preferences from './Settings.svelte';
  import { shouldDrag } from './drag';

  let snapshot = $state<Snapshot>(empty);
  let settings = $state<Settings>(defaults);
  let view = $state<'summary' | 'details' | 'sessions' | 'settings'>('summary');
  let selected = $state<string | null>(null);
  let now = $state(Date.now());
  let refreshing = $state(false);
  let error = $state('');
  let resized = $state(false);
  let fitting = 0;
  let content: HTMLElement;
  const session = $derived(
    snapshot.sessions.find((s) => s.id === (settings.pinnedSession || selected)) ||
      snapshot.sessions[0],
  );
  const bucket = $derived(
    snapshot.quotas.find((b) => b.id === settings.selectedBucket) ||
      (snapshot.quotas.length === 1 ? snapshot.quotas[0] : undefined),
  );
  const status = $derived(session?.activity.value || 'unknown');
  const connectionName = $derived(connectionLabel(snapshot.connection, snapshot.provider));

  async function accept(next: Snapshot) {
    if (next.revision <= snapshot.revision) return;
    if (snapshot.revision && next.revision > snapshot.revision + 1) {
      const current = await command<Snapshot>('snapshot');
      if (current.revision > snapshot.revision) snapshot = current;
    } else snapshot = next;
    if (settings.autoFollow && !settings.pinnedSession && view === 'summary')
      selected = snapshot.sessions[0]?.id || null;
  }
  async function apply(next: Settings) {
    await save(next);
    settings = next;
    error = '';
  }
  async function update(patch: Partial<Settings>) {
    try {
      await apply({ ...settings, ...patch });
    } catch {
      error = 'Could not save settings';
    }
  }
  async function refresh() {
    if (refreshing || snapshot.refreshing) return;
    refreshing = true;
    error = '';
    try {
      await command('refresh');
    } catch {
      error = 'Refresh failed';
    } finally {
      refreshing = false;
    }
  }
  function dragCard(event: PointerEvent) {
    if (shouldDrag(event)) void drag();
  }
  function resizeCard(event: PointerEvent) {
    if (event.button !== 0 || !event.isPrimary) return;
    event.stopPropagation();
    resized = true;
    void resizeHeight()
      .then((started) => {
        if (!started) resized = false;
      })
      .catch(() => {
        resized = false;
      });
  }
  function showView(next: typeof view) {
    resized = false;
    view = next;
  }
  function fitCard(collapsed = settings.collapsed, currentView = view) {
    if (!content) return;
    const panel = !collapsed && currentView !== 'summary';
    if (!collapsed && resized) return;
    fitting += 1;
    void fit(
      collapsed ? 240 : panel ? 360 : 300,
      panel ? 480 : Math.max(40, Math.ceil(content.scrollHeight)),
    )
      .catch(() => {})
      .finally(() => {
        requestAnimationFrame(() => {
          fitting -= 1;
        });
      });
  }
  function followWindow() {
    if (!native || settings.collapsed || resized || !content) return;
    const fillsWindow = Math.abs(content.getBoundingClientRect().height - innerHeight) <= 1;
    if (fitting === 0 || !fillsWindow) resized = true;
  }
  onMount(() => {
    let dispose = () => {};
    let stopped = false;
    const timer = setInterval(() => {
      now = Date.now();
    }, 1000);
    const observer = new ResizeObserver(() => fitCard());
    observer.observe(content);
    window.addEventListener('resize', followWindow);
    void (async () => {
      dispose = await subscribe(
        (next) => {
          void accept(next).catch(() => {});
        },
        () => {
          showView('settings');
        },
      );
      if (stopped) {
        dispose();
        return;
      }
      const [state, preferences] = await Promise.all([
        command<Snapshot>('snapshot'),
        command<Settings>('preferences'),
      ]);
      if (stopped) return;
      if (state.revision >= snapshot.revision) snapshot = state;
      settings = preferences;
    })().catch(() => {
      snapshot = { ...empty, connection: 'unavailable' };
    });
    return () => {
      stopped = true;
      dispose();
      observer.disconnect();
      window.removeEventListener('resize', followWindow);
      clearInterval(timer);
    };
  });
  $effect(() => {
    document.documentElement.dataset.theme = settings.theme;
    document.documentElement.style.fontSize = `${settings.fontSize}px`;
  });
  $effect(() => {
    fitCard(settings.collapsed, view);
  });
</script>

<main
  bind:this={content}
  class:collapsed={settings.collapsed}
  class:panel={!settings.collapsed && view !== 'summary'}
  class:resized
  onpointerdown={dragCard}
>
  {#if settings.collapsed}
    <div class="collapsed-row">
      <span class="status-icon" data-status={status}><Activity size={16} /></span>
      <strong class="truncate" title={session?.path}>{session?.project || 'Codex Status'}</strong>
      <span class="numeric">{percent(bucket?.windows[0]?.remaining.value)}</span>
      <button
        class="icon"
        aria-label="Expand"
        title="Expand"
        onclick={() => update({ collapsed: false })}><ChevronDown size={16} /></button
      >
    </div>
  {:else}
    <header>
      <strong class="brand">Codex Status</strong>
      {#if settings.muted}<BellOff size={14} aria-label="Notifications muted" />{/if}
      <button
        class="icon"
        class:spinning={refreshing || snapshot.refreshing}
        disabled={refreshing || snapshot.refreshing}
        aria-label="Refresh"
        title="Refresh"
        onclick={refresh}><RefreshCw size={15} /></button
      >
      <button
        class="icon"
        aria-label="Settings"
        title="Settings"
        aria-pressed={view === 'settings'}
        onclick={() => {
          showView(view === 'settings' ? 'summary' : 'settings');
        }}><SettingsIcon size={16} /></button
      >
      <button
        class="icon"
        aria-label="Collapse"
        title="Collapse"
        onclick={() => {
          resized = false;
          void update({ collapsed: true });
        }}><ChevronUp size={16} /></button
      >
    </header>
    <div class="session-heading">
      <button
        class="session-button"
        onclick={() => {
          showView(view === 'details' ? 'summary' : 'details');
        }}
        title={session?.path}
      >
        <span class="status-icon" data-status={status}
          >{#if status === 'completed'}<CircleCheck
              size={18}
            />{:else if status === 'running'}<Activity
              size={18}
            />{:else if status.startsWith('waiting')}<Clock size={18} />{:else}<CircleHelp
              size={18}
            />{/if}</span
        >
        <strong class="truncate">{session?.project || 'No session'}</strong>
        <span class="status-word">{session ? activity[status] : ''}</span>
      </button>
      <button
        class="icon"
        disabled={!session}
        aria-label={settings.pinnedSession ? 'Unpin session' : 'Pin session'}
        title={settings.pinnedSession ? 'Unpin session' : 'Pin session'}
        onclick={() => update({ pinnedSession: settings.pinnedSession ? null : session?.id })}
        >{#if settings.pinnedSession}<PinOff size={15} />{:else}<Pin size={15} />{/if}</button
      >
    </div>
    {#if bucket}
      <div class="quota-summary">
        {#each bucket.windows.slice(0, 2) as window}<Quota {window} {now} />{/each}
      </div>
    {:else if snapshot.quotas.length > 1}
      <select
        aria-label="Quota bucket"
        value={settings.selectedBucket || ''}
        onchange={(event) => update({ selectedBucket: event.currentTarget.value })}
        ><option value="" disabled>Select a quota bucket</option
        >{#each snapshot.quotas as item}<option value={item.id}>{item.name}</option>{/each}</select
      >
    {:else}<div class="empty-quota">
        <span class="truncate" title={connectionName}>{connectionName}</span>
      </div>{/if}
    <footer>
      <button
        class="text-tool"
        aria-label="Session list"
        aria-pressed={view === 'sessions'}
        onclick={() => {
          showView(view === 'sessions' ? 'summary' : 'sessions');
        }}><Users size={14} /><span>{snapshot.sessions.length} sessions</span></button
      ><span
        class="connection"
        class:warning={snapshot.connection !== 'connected'}
        title={connectionName}><span class="truncate">{connectionName}</span></span
      >
    </footer>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if view !== 'summary'}
      <div class="view-title">
        <span>{{ details: 'Details', sessions: 'Sessions', settings: 'Settings' }[view]}</span
        ><button
          class="icon"
          aria-label="Close panel"
          title="Close"
          onclick={() => {
            showView('summary');
          }}><X size={16} /></button
        >
      </div>
      <div class="scroll-view" data-no-drag>
        {#if view === 'details'}<Details {session} {snapshot} {now} />
        {:else if view === 'settings'}<Preferences {settings} {apply} />
        {:else}
          <div class="sessions">
            {#each snapshot.sessions as item (item.id)}<button
                class="session-item"
                class:active={session?.id === item.id}
                onclick={() => {
                  selected = item.id;
                  if (settings.pinnedSession) void update({ pinnedSession: item.id });
                  showView('details');
                }}
                ><span class="session-name">{item.project || 'Unknown project'}</span><span
                  data-status={item.activity.value || 'unknown'}
                  >{activity[item.activity.value || 'unknown']}</span
                ><span class="session-path">{item.path}</span></button
              >{:else}<p class="empty">No sessions</p>{/each}
          </div>
        {/if}
      </div>
    {/if}
  {/if}
  {#if !settings.collapsed}
    <div
      class="resize-edge"
      role="separator"
      aria-label="Resize height"
      aria-orientation="horizontal"
      data-no-drag
      onpointerdown={resizeCard}
    ></div>
  {/if}
</main>
