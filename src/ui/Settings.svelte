<script lang="ts">
  import { untrack } from 'svelte';
  import { Save, LogOut, Volume2 } from '@lucide/svelte';
  import type { Settings } from '../types/status';
  import { command, native } from '../state/bridge';
  import { playSound, type Sound } from '../state/sound';
  let { settings, apply }: { settings: Settings; apply: (value: Settings) => Promise<void> } =
    $props();
  function copy(value: Settings): Settings {
    return {
      ...value,
      roots: [...value.roots],
      position: value.position ? [...value.position] : null,
    };
  }
  let draft = $state<Settings>(untrack(() => copy(settings)));
  let roots = $state(untrack(() => settings.roots.join('\n')));
  let saving = $state(false);
  let error = $state('');
  async function preview(sound: Sound) {
    error = '';
    try {
      await playSound(sound);
    } catch {
      error = 'Could not play sound';
    }
  }
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    saving = true;
    error = '';
    try {
      await apply({
        ...draft,
        executable: draft.executable.trim(),
        roots: roots
          .split('\n')
          .map((v) => v.trim())
          .filter(Boolean),
      });
    } catch {
      error = 'Could not save settings';
    } finally {
      saving = false;
    }
  }
</script>

<form onsubmit={submit}>
  <h2>Appearance</h2>
  <label class="setting"
    ><span class="setting-label">Theme</span><select bind:value={draft.theme}
      ><option value="system">System</option><option value="light">Light</option><option
        value="dark">Dark</option
      ></select
    ></label
  >
  <label class="setting"
    ><span class="setting-label">Font size</span><input
      type="number"
      min="12"
      max="18"
      bind:value={draft.fontSize}
    /></label
  >
  <label class="setting"
    ><span class="setting-label">Always on top</span><input
      type="checkbox"
      bind:checked={draft.alwaysOnTop}
    /></label
  >
  <label class="setting"
    ><span class="setting-label">Follow recent activity</span><input
      type="checkbox"
      bind:checked={draft.autoFollow}
    /></label
  >
  <h2>Notifications</h2>
  <label class="setting"
    ><span class="setting-label">Status notifications</span><input
      type="checkbox"
      bind:checked={draft.notifications}
    /></label
  >
  <div class="setting">
    <span class="setting-label">Command approval sound</span>
    <div class="setting-actions sound-choice">
      <button
        class="sound-preview"
        type="button"
        disabled={draft.approvalSound === 'off'}
        aria-label="Preview command approval sound"
        onclick={() => preview('approvalBell')}><Volume2 size={14} /></button
      >
      <select aria-label="Command approval sound" bind:value={draft.approvalSound}
        ><option value="off">Off</option><option value="bell">Bell</option></select
      >
    </div>
  </div>
  <div class="setting">
    <span class="setting-label">Task completion sound</span>
    <div class="setting-actions sound-choice">
      <button
        class="sound-preview"
        type="button"
        disabled={draft.completionSound === 'off'}
        aria-label="Preview task completion sound"
        onclick={() =>
          preview(draft.completionSound === 'bell' ? 'completionBell' : 'completionDing')}
        ><Volume2 size={14} /></button
      >
      <select aria-label="Task completion sound" bind:value={draft.completionSound}
        ><option value="off">Off</option><option value="ding">Ding</option><option value="bell"
          >Bell</option
        ></select
      >
    </div>
  </div>
  <div class="setting">
    <span class="setting-label">Quota warning sound</span>
    <div class="setting-actions sound-choice">
      <button
        class="sound-preview"
        type="button"
        disabled={draft.quotaSound === 'off'}
        aria-label="Preview quota warning sound"
        onclick={() => preview(draft.quotaSound === 'battery' ? 'quotaBattery' : 'quotaAlert')}
        ><Volume2 size={14} /></button
      >
      <select aria-label="Quota warning sound" bind:value={draft.quotaSound}
        ><option value="off">Off</option><option value="alert">Alert</option><option value="battery"
          >Battery</option
        ></select
      >
    </div>
  </div>
  <label class="setting"
    ><span class="setting-label">Muted</span><input
      type="checkbox"
      bind:checked={draft.muted}
    /></label
  >
  <label class="setting"
    ><span class="setting-label">Low quota threshold %</span><input
      type="number"
      min="0"
      max="100"
      bind:value={draft.lowQuota}
    /></label
  >
  <h2>Data sources</h2>
  <label class="stack"
    ><span>CLI executable</span><input
      bind:value={draft.executable}
      required
      maxlength="4096"
      spellcheck="false"
    /></label
  >
  <label class="stack"
    ><span>Data roots</span><textarea
      bind:value={roots}
      rows="3"
      spellcheck="false"
      aria-label="Data roots, one per line"></textarea></label
  >
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div class="form-actions">
    <button class="command" type="submit" disabled={saving}
      ><Save size={16} />{saving ? 'Saving' : 'Save'}</button
    >{#if native}<button class="command" type="button" onclick={() => command('quit')}
        ><LogOut size={16} />Quit</button
      >{/if}
  </div>
</form>
