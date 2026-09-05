import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { acquireBuildLock } from './lock.mjs';

test('a build lock excludes another task and is released by its owner', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'codex-status-lock-'));
  const path = join(directory, 'build.lock');
  try {
    const release = await acquireBuildLock(path, '');
    await assert.rejects(acquireBuildLock(path, ''), /Another build is active/);
    await release();
    const nextRelease = await acquireBuildLock(path, '');
    await nextRelease();
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('an inherited lock requires the matching owner token', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'codex-status-lock-'));
  const path = join(directory, 'build.lock');
  try {
    await writeFile(path, 'owner-token');
    await assert.rejects(acquireBuildLock(path, 'other-token'), /Another build is active/);
    assert.equal(await readFile(path, 'utf8'), 'owner-token');
    const release = await acquireBuildLock(path, 'owner-token');
    await release();
    await assert.rejects(readFile(path, 'utf8'), { code: 'ENOENT' });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
