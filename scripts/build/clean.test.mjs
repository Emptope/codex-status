import assert from 'node:assert/strict';
import { access, mkdir, mkdtemp, readFile, rm, symlink, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { cleanCaches, cleanOutputs } from './clean.mjs';

test('task cleanup accepts aliased parents and preserves only flat artifacts', async () => {
  const target = await mkdtemp(join(tmpdir(), 'clean-'));
  const root = `${target}-alias`;
  const build = join(root, 'build');
  const artifact = join(build, 'artifacts', 'status-v1.2.3-windows-x64.exe');
  const legacy = join(build, 'artifacts', 'windows', 'x64', 'status.exe');
  try {
    await symlink(target, root, process.platform === 'win32' ? 'junction' : 'dir');
    await mkdir(join(build, 'web'), { recursive: true });
    await mkdir(join(build, 'staging', 'web'), { recursive: true });
    await mkdir(join(build, 'test', 'results'), { recursive: true });
    await mkdir(join(build, 'cache', 'cargo', 'host', 'debug'), { recursive: true });
    await mkdir(join(build, 'cache', 'cargo', 'host', 'release', 'bundle'), { recursive: true });
    await mkdir(join(build, 'cache', 'vite'), { recursive: true });
    await mkdir(join(build, 'cargo', 'debug'), { recursive: true });
    await mkdir(join(build, 'artifacts', 'windows', 'x64'), { recursive: true });
    await writeFile(join(build, 'web', 'index.html'), 'old');
    await writeFile(join(build, 'staging', 'web', 'index.html'), 'current');
    await writeFile(join(build, 'test', 'results', 'result.json'), '{}');
    await writeFile(join(build, 'cache', 'cargo', 'host', 'debug', 'binary'), 'cache');
    await writeFile(join(build, 'cache', 'cargo', 'host', 'release', 'bundle', 'old.dmg'), 'old');
    await writeFile(join(build, 'cache', 'vite', 'metadata.json'), '{}');
    await writeFile(join(build, 'cargo', 'debug', 'legacy'), 'old');
    await writeFile(artifact, 'current');
    await writeFile(legacy, 'old');
    await cleanOutputs(build, { preserveArtifacts: true });
    assert.equal(await readFile(artifact, 'utf8'), 'current');
    await assert.rejects(access(join(build, 'web')));
    await assert.rejects(access(join(build, 'staging')));
    await assert.rejects(access(join(build, 'test')));
    await assert.rejects(access(join(build, 'cargo')));
    await assert.rejects(access(join(build, 'artifacts', 'windows')));
    await assert.rejects(access(join(build, 'cache', 'cargo', 'host', 'release', 'bundle')));
    assert.equal(
      await readFile(join(build, 'cache', 'cargo', 'host', 'debug', 'binary'), 'utf8'),
      'cache',
    );
    assert.equal(await readFile(join(build, 'cache', 'vite', 'metadata.json'), 'utf8'), '{}');
  } finally {
    await unlink(root).catch(() => {});
    await rm(target, { recursive: true, force: true });
  }
});

test('cache cleanup removes current and legacy caches without touching outputs', async () => {
  const root = await mkdtemp(join(tmpdir(), 'clean-cache-'));
  const build = join(root, 'build');
  try {
    await mkdir(join(build, 'cache', 'cargo'), { recursive: true });
    await mkdir(join(build, 'cargo'), { recursive: true });
    await mkdir(join(build, 'vite-cache'), { recursive: true });
    await mkdir(join(build, 'artifacts'), { recursive: true });
    await writeFile(join(build, 'cache', 'cargo', 'state'), 'current');
    await writeFile(join(build, 'cargo', 'state'), 'legacy');
    await writeFile(join(build, 'vite-cache', 'state'), 'legacy');
    await writeFile(join(build, 'artifacts', 'status.exe'), 'output');

    await cleanCaches(build);

    await assert.rejects(access(join(build, 'cache')));
    await assert.rejects(access(join(build, 'cargo')));
    await assert.rejects(access(join(build, 'vite-cache')));
    assert.equal(await readFile(join(build, 'artifacts', 'status.exe'), 'utf8'), 'output');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('default output cleanup removes previous release artifacts', async () => {
  const root = await mkdtemp(join(tmpdir(), 'clean-artifacts-'));
  const build = join(root, 'build');
  try {
    await mkdir(join(build, 'artifacts'), { recursive: true });
    await writeFile(join(build, 'artifacts', 'old.dmg'), 'old');

    await cleanOutputs(build);

    await assert.rejects(access(join(build, 'artifacts')));
  } finally {
    await rm(root, { recursive: true, force: true });
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
