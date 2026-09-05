import { open } from 'node:fs/promises';

export const supportedVersions = new Set(['0.153.4']);
const numeric = (value) => (Number.isSafeInteger(value) && value >= 0 ? value : null);

export function account(result) {
  if (result?.account === null)
    return result.requiresOpenaiAuth === false ? 'externalProvider' : 'signedOut';
  const type = result?.account?.type;
  if (['apiKey', 'chatgpt', 'amazonBedrock'].includes(type)) return type;
  return type ? 'unsupported' : 'unavailable';
}

export function quota(window, now = Date.now() / 1000) {
  if (!window || typeof window !== 'object') return null;
  const used = numeric(window.usedPercent);
  const resetsAt = numeric(window.resetsAt);
  const valid = used !== null && used <= 100;
  return {
    remainingPercent: valid ? 100 - used : null,
    windowMinutes: numeric(window.windowDurationMins),
    resetsAt,
    quality: !valid ? 'unavailable' : resetsAt !== null && resetsAt <= now ? 'stale' : 'fresh',
  };
}

function usage(value) {
  if (!value || typeof value !== 'object') return null;
  return {
    inputTokens: numeric(value.input_tokens),
    cachedInputTokens: numeric(value.cached_input_tokens),
    outputTokens: numeric(value.output_tokens),
    totalTokens: numeric(value.total_tokens),
  };
}

export function summarize(rows, previous) {
  const state = previous
    ? structuredClone(previous)
    : {
        version: null,
        supported: false,
        status: 'unknown',
        observedAt: null,
        statusAt: null,
        usageAt: null,
        turnId: null,
        usage: null,
        lastUsage: null,
        events: {},
      };
  for (const row of rows) {
    if (!row || typeof row !== 'object') continue;
    const p = row.payload;
    if (!p || typeof p !== 'object') continue;
    if (row.type === 'session_meta') {
      state.version =
        typeof p.cli_version === 'string' && /^\d+\.\d+\.\d+$/.test(p.cli_version)
          ? p.cli_version
          : null;
      state.supported = supportedVersions.has(state.version);
      continue;
    }
    if (!state.supported) continue;
    const at = Date.parse(row.timestamp);
    if (!Number.isFinite(at)) continue;
    const observed = new Date(at).toISOString();
    if (!state.observedAt || at > Date.parse(state.observedAt)) state.observedAt = observed;
    if (row.type !== 'event_msg') continue;
    const kind = p.type;
    if (['task_started', 'task_complete', 'turn_aborted', 'token_count'].includes(kind)) {
      state.events[kind] = (state.events[kind] || 0) + 1;
    }
    const statuses = {
      task_started: 'running',
      task_complete: 'completed',
      turn_aborted: 'interrupted',
    };
    if (
      Object.hasOwn(statuses, kind) &&
      typeof p.turn_id === 'string' &&
      (!state.statusAt || at >= Date.parse(state.statusAt))
    ) {
      if (kind !== 'task_started' && state.turnId && state.turnId !== p.turn_id) continue;
      state.status = statuses[kind];
      state.statusAt = observed;
      state.turnId = p.turn_id;
    }
    if (kind === 'token_count' && (!state.usageAt || at >= Date.parse(state.usageAt))) {
      const total = usage(p.info?.total_token_usage);
      if (total) {
        state.usage = total;
        state.lastUsage = usage(p.info?.last_token_usage);
        state.usageAt = observed;
      }
    }
  }
  return state;
}

export class Cursor {
  #offset = 0;
  #identity = null;
  #partial = Buffer.alloc(0);
  #discarding = false;
  #maxLineBytes;
  #maxReadBytes;

  constructor({ maxLineBytes = 1024 * 1024, maxReadBytes = 256 * 1024 } = {}) {
    this.#maxLineBytes = maxLineBytes;
    this.#maxReadBytes = maxReadBytes;
  }

  async read(path) {
    let handle;
    try {
      handle = await open(path, 'r');
    } catch (error) {
      if (error.code === 'ENOENT') {
        this.#identity = null;
        return { rows: [], missing: true, bytesRead: 0, more: false };
      }
      throw new Error('record-unreadable');
    }
    try {
      const stat = await handle.stat();
      if (!stat.isFile()) return { rows: [], bytesRead: 0, more: false };
      const identity = `${stat.dev}:${stat.ino}:${stat.birthtimeMs}`;
      const reset = this.#identity !== identity || stat.size < this.#offset;
      if (reset) {
        this.#offset = 0;
        this.#partial = Buffer.alloc(0);
        this.#discarding = false;
        this.#identity = identity;
      }
      const bytes = Buffer.alloc(
        Math.min(this.#maxReadBytes, Math.max(0, stat.size - this.#offset)),
      );
      const { bytesRead } = await handle.read(bytes, 0, bytes.length, this.#offset);
      this.#offset += bytesRead;
      let buffer = Buffer.concat([this.#partial, bytes.subarray(0, bytesRead)]);
      const rows = [];
      let invalidLines = 0;
      let end;
      while ((end = buffer.indexOf(10)) !== -1) {
        const line = buffer.subarray(0, end);
        buffer = buffer.subarray(end + 1);
        if (this.#discarding || line.length > this.#maxLineBytes) {
          this.#discarding = false;
          invalidLines++;
          continue;
        }
        try {
          rows.push(JSON.parse(line.toString('utf8')));
        } catch {
          invalidLines++;
        }
      }
      if (buffer.length > this.#maxLineBytes) this.#discarding = true;
      this.#partial = this.#discarding ? Buffer.alloc(0) : Buffer.from(buffer);
      return { rows, reset, invalidLines, bytesRead, more: this.#offset < stat.size };
    } finally {
      await handle.close();
    }
  }
}
