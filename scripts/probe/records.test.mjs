import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile, appendFile, rename } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { Cursor, summarize, quota, account } from './records.mjs';

test('missing quota differs from zero and expired quota never resets optimistically', () => {
  assert.equal(quota(null, 100), null);
  assert.deepEqual(quota({ usedPercent: 100, windowDurationMins: 45, resetsAt: 200 }, 100), {
    remainingPercent: 0,
    windowMinutes: 45,
    resetsAt: 200,
    quality: 'fresh',
  });
  assert.equal(quota({ usedPercent: 0, resetsAt: 99 }, 100).quality, 'stale');
  assert.equal(quota({ usedPercent: -1 }, 100).remainingPercent, null);
  assert.equal(quota({ usedPercent: 101 }, 100).remainingPercent, null);
  assert.equal(quota({ usedPercent: 0, resetsAt: 200 }, 100).remainingPercent, 100);
});

test('account modes are distinct and identity is not exported', () => {
  assert.equal(account({ account: null, requiresOpenaiAuth: true }), 'signedOut');
  assert.equal(account({ account: { type: 'apiKey', apiKey: 'secret' } }), 'apiKey');
  assert.equal(account({ account: { type: 'chatgpt', email: 'private' } }), 'chatgpt');
  assert.equal(account({ account: { type: 'future' } }), 'unsupported');
  assert.equal(account({}), 'unavailable');
});

test('only explicit terminal events establish status; token snapshots replace', () => {
  const rows = [
    {
      type: 'session_meta',
      payload: {
        id: 'id',
        cli_version: '0.153.4',
        originator: 'codex-tui',
        instructions: 'secret',
      },
    },
    {
      timestamp: '2026-09-05T00:00:00Z',
      type: 'event_msg',
      payload: { type: 'task_started', turn_id: 'turn' },
    },
    {
      timestamp: '2026-09-05T00:00:01Z',
      type: 'event_msg',
      payload: {
        type: 'token_count',
        info: {
          total_token_usage: {
            input_tokens: 100,
            cached_input_tokens: 50,
            output_tokens: 10,
            total_tokens: 110,
          },
          model_context_window: 1000,
        },
      },
    },
    {
      timestamp: '2026-09-05T00:00:02Z',
      type: 'event_msg',
      payload: { type: 'task_complete', turn_id: 'turn', last_agent_message: 'secret' },
    },
  ];
  const state = summarize(rows);
  assert.equal(state.status, 'completed');
  assert.equal(state.usage.totalTokens, 110);
  assert.equal(summarize([...rows, rows[2]]).usage.totalTokens, 110);
  assert.equal(summarize([...rows, rows[1]]).status, 'completed');
  assert.equal(JSON.stringify(state).includes('secret'), false);
  assert.equal('contextPercent' in state, false);
  assert.equal(summarize(rows.slice(0, 2)).status, 'running');
  assert.equal(summarize([rows[0]]).status, 'unknown');
  assert.equal(
    summarize([{ ...rows[0], payload: { ...rows[0].payload, cli_version: '99.0.0' } }, rows[1]])
      .status,
    'unknown',
  );
});

test('incremental reading handles partial UTF-8, corrupt lines, truncation and replacement', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'status-records-'));
  const file = join(dir, 'record');
  try {
    const line = Buffer.from(JSON.stringify({ text: '\u4e2d\u6587' }) + '\n');
    const split = line.indexOf(Buffer.from('\u4e2d')) + 1;
    await writeFile(file, line.subarray(0, split));
    const cursor = new Cursor();
    assert.deepEqual((await cursor.read(file)).rows, []);
    await appendFile(file, line.subarray(split));
    assert.deepEqual((await cursor.read(file)).rows, [{ text: '\u4e2d\u6587' }]);
    assert.deepEqual((await cursor.read(file)).rows, []);
    await appendFile(file, 'broken\n{"ok":true}\n');
    const damaged = await cursor.read(file);
    assert.equal(damaged.invalidLines, 1);
    assert.deepEqual(damaged.rows, [{ ok: true }]);
    await writeFile(file, '{}\n');
    assert.equal((await cursor.read(file)).reset, true);
    await writeFile(join(dir, 'replacement'), '{"new":true}\n');
    await rename(join(dir, 'replacement'), file);
    assert.deepEqual((await cursor.read(file)).rows, [{ new: true }]);
    await rm(file);
    assert.equal((await cursor.read(file)).missing, true);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('oversized records are discarded through newline before parsing resumes', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'status-bounds-'));
  const file = join(dir, 'record');
  try {
    await writeFile(file, 'x'.repeat(300) + '\n{}\n');
    const cursor = new Cursor({ maxLineBytes: 64, maxReadBytes: 100 });
    const rows = [];
    for (let i = 0; i < 4; i++) rows.push(...(await cursor.read(file)).rows);
    assert.deepEqual(rows, [{}]);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
