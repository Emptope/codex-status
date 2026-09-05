import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { artifactName, executableName, stageExecutable } from './release.mjs';

test('release artifact names identify their platform and architecture', () => {
  assert.equal(executableName('status', 'win32'), 'status.exe');
  assert.equal(executableName('status', 'linux'), 'status');
  assert.equal(artifactName('status', 'win32', 'x64'), 'status-win32-x64.exe');
  assert.equal(artifactName('status', 'darwin', 'arm64'), 'status-darwin-arm64');
});

test('a single release executable is staged outside the Cargo cache', async () => {
  const root = await mkdtemp(join(tmpdir(), 'release-'));
  const target = join(root, 'build', 'cargo', 'host');
  try {
    await mkdir(join(target, 'release'), { recursive: true });
    await writeFile(join(root, 'package.json'), JSON.stringify({ name: 'status' }));
    await writeFile(join(target, 'release', 'status'), 'portable');
    const artifact = await stageExecutable(root, target, 'linux', 'x64');
    assert.equal(artifact.name, 'status-linux-x64');
    assert.equal(await readFile(artifact.path, 'utf8'), 'portable');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
