import { createHash } from 'node:crypto';
import { readdir, stat } from 'node:fs/promises';
import { homedir, platform, release } from 'node:os';
import { join, resolve } from 'node:path';
import { parseArgs } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import { Rpc } from './rpc.mjs';
import { Cursor, summarize, quota, account } from './records.mjs';

const { values } = parseArgs({
  options: {
    'data-root': { type: 'string' },
    executable: { type: 'string', default: 'codex' },
    'local-only': { type: 'boolean', default: false },
    seconds: { type: 'string', default: '0' },
  },
});
const seconds = Number(values.seconds);
if (!Number.isFinite(seconds) || seconds < 0 || seconds > 60) throw new Error('invalid-duration');
const root = resolve(values['data-root'] || process.env.CODEX_HOME || join(homedir(), '.codex'));
const hash = (value) => createHash('sha256').update(value).digest('hex').slice(0, 16);
const report = {
  observedAt: new Date().toISOString(),
  environment: {
    platform: platform(),
    release: release(),
    wsl: !!process.env.WSL_DISTRO_NAME,
    rootId: hash(root),
  },
  rpc: { status: 'notRun' },
  local: { files: 0, bytesRead: 0, invalidLines: 0, limited: false, sessions: [] },
};
const files = [];
async function discover(dir, depth = 0) {
  if (depth > 8 || files.length >= 10000) {
    report.local.limited = true;
    return;
  }
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const entry of entries) {
    if (files.length >= 10000) {
      report.local.limited = true;
      break;
    }
    const path = join(dir, entry.name);
    if (entry.isDirectory()) await discover(path, depth + 1);
    else if (entry.isFile()) {
      const info = await stat(path).catch(() => null);
      if (info) files.push({ path, modified: info.mtimeMs });
    }
  }
}

await discover(join(root, 'sessions'));
report.local.files = files.length;
files.sort((a, b) => b.modified - a.modified);
const watched = files
  .slice(0, 10)
  .map((file) => ({ ...file, cursor: new Cursor(), state: undefined }));
async function scan() {
  for (const file of watched) {
    let remaining = 32 * 1024 * 1024;
    let result;
    do {
      result = await file.cursor.read(file.path);
      report.local.bytesRead += result.bytesRead;
      report.local.invalidLines += result.invalidLines || 0;
      if (result.reset) file.state = undefined;
      file.state = summarize(result.rows, file.state);
      remaining -= result.bytesRead;
    } while (result.more && remaining > 0);
    if (result.more) report.local.limited = true;
  }
}
await scan();
const before = watched.map((file) => file.state?.observedAt);
if (seconds) {
  const deadline = Date.now() + seconds * 1000;
  while (Date.now() < deadline) {
    await delay(Math.min(500, deadline - Date.now()));
    await scan();
  }
}
report.local.sessions = watched
  .filter((file) => file.state?.session)
  .map((file, index) => ({
    fileId: hash(file.path),
    ...file.state,
    turnId: file.state.turnId ? hash(file.state.turnId) : null,
    changedDuringObservation: before[index] !== file.state.observedAt,
  }));

if (!values['local-only']) {
  const rpc = Rpc.start(values.executable, { ...process.env, CODEX_HOME: root });
  const stop = () => {
    void rpc.close();
  };
  process.once('SIGINT', stop);
  process.once('SIGTERM', stop);
  try {
    await rpc.initialize();
    report.rpc.status = 'connected';
    report.rpc.account = account(await rpc.call('account/read'));
    try {
      const limits = await rpc.call('account/rateLimits/read');
      report.rpc.quotas = Object.entries(
        limits.rateLimitsByLimitId || { default: limits.rateLimits },
      ).map(([id, bucket]) => ({
        bucketId: hash(id),
        primary: quota(bucket?.primary),
        secondary: quota(bucket?.secondary),
      }));
    } catch (error) {
      report.rpc.quotaError = { kind: error.message, code: error.code ?? null };
    }
    const threads = await rpc.call('thread/list');
    report.rpc.threads = (threads.data || []).map((thread) => ({
      id: hash(thread.id),
      status: ['notLoaded', 'idle', 'systemError', 'active'].includes(thread.status?.type)
        ? thread.status.type
        : 'unknown',
    }));
    if (threads.data?.[0]?.id) {
      const read = await rpc.call('thread/read', { threadId: threads.data[0].id });
      report.rpc.metadataRead = !!read.thread;
    }
  } catch (error) {
    report.rpc.status = 'unavailable';
    report.rpc.error = { kind: error.message, code: error.code ?? null };
  } finally {
    await rpc.close();
    process.removeListener('SIGINT', stop);
    process.removeListener('SIGTERM', stop);
    report.rpc.methods = rpc.methods;
    report.rpc.unansweredServerRequests = rpc.serverRequests;
    report.rpc.stderrBytes = rpc.stderrBytes;
    report.rpc.closed = true;
  }
}
console.log(JSON.stringify(report, null, 2));
