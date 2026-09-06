import assert from 'node:assert/strict';
import { access, mkdir, mkdtemp, readFile, rm, symlink, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { cleanOutputs } from './clean.mjs';

test('task cleanup accepts aliased parents and preserves only flat artifacts', async () => {
  const target = await mkdtemp(join(tmpdir(), 'clean-'));
  const root = `${target}-alias`;
  const build = join(root, 'build');
  const artifact = join(build, 'artifacts', 'status-v1.2.3-windows-x64.exe');
  const legacy = join(build, 'artifacts', 'windows', 'x64', 'status.exe');
  try {
    await symlink(target, root, process.platform === 'win32' ? 'junction' : 'dir');
    await mkdir(join(build, 'web'), { recursive: true });
    await mkdir(join(build, 'artifacts', 'windows', 'x64'), { recursive: true });
    await writeFile(join(build, 'web', 'index.html'), 'old');
    await writeFile(artifact, 'current');
    await writeFile(legacy, 'old');
    await cleanOutputs(build, { preserveArtifacts: true });
    assert.equal(await readFile(artifact, 'utf8'), 'current');
    await assert.rejects(access(join(build, 'web')));
    await assert.rejects(access(join(build, 'artifacts', 'windows')));
  } finally {
    await unlink(root).catch(() => {});
    await rm(target, { recursive: true, force: true });
  }
});

test('task cleanup rejects a symbolic build directory', async () => {
  const target = await mkdtemp(join(tmpdir(), 'clean-target-'));
  const build = `${target}-link`;
  try {
    await symlink(target, build, process.platform === 'win32' ? 'junction' : 'dir');
    await assert.rejects(cleanOutputs(build), /Unsafe build directory/);
  } finally {
    await unlink(build).catch(() => {});
    await rm(target, { recursive: true, force: true });
  }
});
