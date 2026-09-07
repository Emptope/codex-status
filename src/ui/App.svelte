<script lang="ts">
  import { onMount } from 'svelte';
  import brandIcon from '../../src-tauri/icons/icon.svg?url';
  import {
    ChevronDown,
    ChevronUp,
    RefreshCw,
    Settings as SettingsIcon,
    X,
    Users,
    BellOff,
  } from '@lucide/svelte';
  import { empty, defaults, type Snapshot, type Settings } from '../types/status';
  import {
    activity,
    connectionLabel,
    error as errorLabel,
    nextCountdownUpdate,
    percent,
    unit,
  } from '../state/format';
  import { command, drag, fit, native, resizeHeight, save, subscribe } from '../state/bridge';
  import { playSound } from '../state/sound';
  import Quota from './Quota.svelte';
  import Details from './Details.svelte';
  import Preferences from './Settings.svelte';
  import { shouldDrag } from './drag';
  import {
    draggedHeight,
    framedHeight,
    framedWidth,
    shadowInsets,
    viewWidth,
    type View,
  } from './layout';

  let snapshot = $state<Snapshot>(empty);
  let settings = $state<Settings>(defaults);
  let view = $state<View>('summary');
  let selected = $state<string | null>(null);
  let now = $state(Date.now());
  let refreshing = $state(false);
  let error = $state('');
  let resized = $state(false);
  let fitting = 0;
  let resizeFrame = 0;
  let resizeValue = 0;
  let clockTimer = 0;
  let clockResets: Array<number | null> = [];
  let mounted = false;
  let resizing: { pointer: number; y: number; height: number } | null = null;
  let content: HTMLElement;
  const session = $derived(
    snapshot.sessions.find((s) => s.id === selected) || snapshot.sessions[0],
  );
  const bucket = $derived(
    snapshot.quotas.find((b) => b.id === settings.selectedBucket) ||
      (snapshot.quotas.length === 1 ? snapshot.quotas[0] : undefined),
  );
  const status = $derived(session?.activity.value || 'unknown');
  const connectionName = $derived(connectionLabel(snapshot.connection, snapshot.provider));

  function accept(next: Snapshot) {
    if (next.revision <= snapshot.revision) return;
    snapshot = next;
    if (settings.autoFollow && view === 'summary') selected = snapshot.sessions[0]?.id || null;
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
    if (refreshing) return;
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
    if (!native || event.button !== 0 || !event.isPrimary) return;
    const edge = event.currentTarget as HTMLElement;
    event.preventDefault();
    event.stopPropagation();
    edge.setPointerCapture(event.pointerId);
    resizing = { pointer: event.pointerId, y: event.screenY, height: innerHeight };
    resized = true;
  }
  function resizeCardMove(event: PointerEvent) {
    if (!resizing || event.pointerId !== resizing.pointer) return;
    resizeValue = draggedHeight(resizing.height, resizing.y, event.screenY, framedHeight(40));
    if (resizeFrame) return;
    resizeFrame = requestAnimationFrame(() => {
      resizeFrame = 0;
      void resizeHeight(resizeValue).catch(() => {
        resized = false;
        resizing = null;
      });
    });
  }
  function resizeCardEnd(event: PointerEvent) {
    if (!resizing || event.pointerId !== resizing.pointer) return;
    const edge = event.currentTarget as HTMLElement;
    if (edge.hasPointerCapture(event.pointerId)) {
      edge.releasePointerCapture(event.pointerId);
    }
    resizing = null;
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
    const width = collapsed ? 240 : viewWidth(currentView);
    const height = panel ? 480 : Math.max(40, Math.ceil(content.scrollHeight));
    void fit(framedWidth(width), framedHeight(height))
      .catch(() => {})
      .finally(() => {
        requestAnimationFrame(() => {
          fitting -= 1;
        });
      });
  }
  function followWindow() {
    if (!native || settings.collapsed || resized || !content) return;
    const fillsWindow =
      Math.abs(framedHeight(content.getBoundingClientRect().height) - innerHeight) <= 1;
    if (fitting === 0 || !fillsWindow) resized = true;
  }
  function scheduleClock() {
    if (clockTimer) clearTimeout(clockTimer);
    clockTimer = 0;
    const current = Date.now();
    now = current;
    if (!mounted || document.hidden) return;
    const delay = nextCountdownUpdate(clockResets, current);
    if (delay !== null) {
      clockTimer = window.setTimeout(scheduleClock, Math.min(delay, 2_147_483_647));
    }
  }
  function followVisibility() {
    scheduleClock();
  }
  onMount(() => {
    let dispose = () => {};
    let stopped = false;
    mounted = true;
    scheduleClock();
    const observer = new ResizeObserver(() => fitCard());
    observer.observe(content);
    window.addEventListener('resize', followWindow);
    document.addEventListener('visibilitychange', followVisibility);
    void (async () => {
      dispose = await subscribe(
        (next) => {
          accept(next);
        },
        () => {
          showView('settings');
        },
        (sound) => {
          void playSound(sound).catch(() => {});
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
      mounted = false;
      dispose();
      observer.disconnect();
      window.removeEventListener('resize', followWindow);
      document.removeEventListener('visibilitychange', followVisibility);
      if (clockTimer) clearTimeout(clockTimer);
      if (resizeFrame) cancelAnimationFrame(resizeFrame);
    };
  });
  $effect(() => {
    document.documentElement.dataset.theme = settings.theme;
    document.documentElement.style.fontSize = `${settings.fontSize}px`;
  });
  $effect(() => {
    fitCard(settings.collapsed, view);
  });
  $effect(() => {
    clockResets = settings.collapsed
      ? []
      : bucket?.windows.slice(0, 2).map((window) => window.resetsAt) || [];
    if (mounted) scheduleClock();
  });
</script>

<main
  bind:this={content}
  class:framed={native}
  class:collapsed={settings.collapsed}
  class:panel={!settings.collapsed && view !== 'summary'}
  class:resized
  style={`--shadow-x: ${shadowInsets.horizontal}px; --shadow-top: ${shadowInsets.top}px; --shadow-bottom: ${shadowInsets.bottom}px`}
  onpointerdown={dragCard}
>
  {#if settings.collapsed}
    <div class="collapsed-row">
      <strong class="truncate" title={session?.path}>{session?.project || 'Codex Status'}</strong>
      {#if session}<span class="collapsed-status" data-status={status} title={activity[status]}
          >{activity[status]}</span
        >{/if}
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
      <strong class="brand"><img class="brand-icon" src={brandIcon} alt="" />Codex Status</strong>
      {#if settings.muted}<BellOff size={14} aria-label="Notifications muted" />{/if}
      <button
        class="icon"
        class:spinning={refreshing || snapshot.refreshing}
        disabled={refreshing}
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
        <strong class="truncate">{session?.project || 'No session'}</strong>
        <span class="status-word">{session ? activity[status] : ''}</span>
      </button>
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
        }}><Users size={14} /><span>{unit(snapshot.sessions.length, 'session')}</span></button
      ><span
        class="connection"
        class:warning={snapshot.connection !== 'connected'}
        title={connectionName}><span class="truncate">{connectionName}</span></span
      >
    </footer>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if snapshot.error || snapshot.localError}<p class="error source-error" role="status">
        {errorLabel[snapshot.error || snapshot.localError || ''] || 'Source unavailable'}
      </p>{/if}
    {#if view !== 'summary'}
      <div class="view-title">
        <span
          >{{ details: 'Session details', sessions: 'Sessions', settings: 'Settings' }[view]}</span
        ><button
          class="icon"
          aria-label="Close panel"
          title="Close"
          onclick={() => {
            showView('summary');
          }}><X size={16} /></button
        >
      </div>
      <div class="scroll-view" class:settings-view={view === 'settings'} data-no-drag>
        {#if view === 'details'}<Details {session} />
        {:else if view === 'settings'}<Preferences {settings} {apply} />
        {:else}
          <div class="sessions">
            {#each snapshot.sessions as item (item.id)}<button
                class="session-item"
                class:active={session?.id === item.id}
                onclick={() => {
                  selected = item.id;
                  showView('details');
                }}
                ><span class="session-name">{item.project || 'Unknown project'}</span><span
                  class="session-status"
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
      onpointermove={resizeCardMove}
      onpointerup={resizeCardEnd}
      onpointercancel={resizeCardEnd}
      onlostpointercapture={resizeCardEnd}
    ></div>
  {/if}
</main>
