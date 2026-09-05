<script lang="ts">
  import { untrack } from 'svelte';
  import { Save, LogOut } from '@lucide/svelte';
  import type { Settings } from '../types/status';
  import { command, native } from '../state/bridge';
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
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    saving = true;
    error = '';
    try {
      await apply({
        ...draft,
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
    >Theme<select bind:value={draft.theme}
      ><option value="system">System</option><option value="light">Light</option><option
        value="dark">Dark</option
      ></select
    ></label
  >
  <label class="setting"
    >Font size<input type="number" min="12" max="18" bind:value={draft.fontSize} /></label
  >
  <label class="setting"
    >Always on top<input type="checkbox" bind:checked={draft.alwaysOnTop} /></label
  >
  <label class="setting"
    >Follow recent activity<input type="checkbox" bind:checked={draft.autoFollow} /></label
  >
  <h2>Notifications</h2>
  <label class="setting"
    >Status notifications<input type="checkbox" bind:checked={draft.notifications} /></label
  >
  <label class="setting">Muted<input type="checkbox" bind:checked={draft.muted} /></label>
  <label class="setting"
    >Low quota threshold %<input
      type="number"
      min="0"
      max="100"
      bind:value={draft.lowQuota}
    /></label
  >
  <h2>Data sources</h2>
  <label class="stack"
    >CLI executable<input
      bind:value={draft.executable}
      required
      maxlength="4096"
      spellcheck="false"
    /></label
  >
  <label class="stack"
    >Data roots<textarea
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
