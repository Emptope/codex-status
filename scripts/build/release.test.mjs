import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { artifactPath, executableName, stageExecutable } from './release.mjs';

test('release artifacts are separated by platform and architecture', () => {
  assert.equal(executableName('status', 'win32'), 'status.exe');
  assert.equal(executableName('status', 'linux'), 'status');
  assert.equal(
    artifactPath('/repo', 'status', 'win32', 'x64'),
    join('/repo', 'build', 'artifacts', 'windows', 'x64', 'status.exe'),
  );
  assert.equal(
    artifactPath('/repo', 'status', 'linux', 'x64'),
    join('/repo', 'build', 'artifacts', 'linux', 'x64', 'status'),
  );
  assert.equal(
    artifactPath('/repo', 'status', 'darwin', 'arm64'),
    join('/repo', 'build', 'artifacts', 'macos', 'arm64', 'status'),
  );
});

test('a single release executable is staged outside the Cargo cache', async () => {
  const root = await mkdtemp(join(tmpdir(), 'release-'));
  const target = join(root, 'build', 'cargo', 'host');
  try {
    await mkdir(join(target, 'release'), { recursive: true });
    await writeFile(join(root, 'package.json'), JSON.stringify({ name: 'status' }));
    await writeFile(join(target, 'release', 'status'), 'portable');
    const artifact = await stageExecutable(root, target, 'linux', 'x64');
    assert.equal(artifact.name, 'status');
    assert.equal(artifact.path, join(root, 'build', 'artifacts', 'linux', 'x64', 'status'));
    assert.equal(await readFile(artifact.path, 'utf8'), 'portable');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
