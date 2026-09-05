import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import {
  artifactPath,
  clearArtifact,
  executableName,
  releaseName,
  stageExecutable,
} from './release.mjs';

test('release artifacts are separated by platform and architecture', () => {
  assert.equal(executableName('status', 'win32'), 'status.exe');
  assert.equal(executableName('status', 'linux'), 'status');
  assert.equal(
    artifactPath('/repo', 'status', '1.2.3', 'win32', 'x64'),
    join('/repo', 'build', 'artifacts', 'status-v1.2.3-windows-x64.exe'),
  );
  assert.equal(
    artifactPath('/repo', 'status', '1.2.3', 'linux', 'x64'),
    join('/repo', 'build', 'artifacts', 'status-v1.2.3-linux-x64'),
  );
  assert.equal(
    artifactPath('/repo', 'status', '1.2.3', 'darwin', 'arm64'),
    join('/repo', 'build', 'artifacts', 'status-v1.2.3-macos-arm64'),
  );
});

test('a release executable and checksum are staged in the unified artifact directory', async () => {
  const root = await mkdtemp(join(tmpdir(), 'release-'));
  const target = join(root, 'build', 'cargo', 'host');
  try {
    await mkdir(join(target, 'release'), { recursive: true });
    await writeFile(
      join(root, 'package.json'),
      JSON.stringify({ name: 'status', version: '1.2.3' }),
    );
    await writeFile(join(target, 'release', 'status'), 'portable');
    const artifact = await stageExecutable(root, target, 'linux', 'x64');
    assert.equal(artifact.name, 'status-v1.2.3-linux-x64');
    assert.equal(artifact.path, join(root, 'build', 'artifacts', 'status-v1.2.3-linux-x64'));
    assert.equal(await readFile(artifact.path, 'utf8'), 'portable');
    assert.equal(
      await readFile(artifact.checksumPath, 'utf8'),
      '01e782826ae5182220bd6158f883d01ceb1bce659dc020e7c511f802a9aa7737  status-v1.2.3-linux-x64\n',
    );
    assert.equal(releaseName('status', '1.2.3', 'win32', 'x64'), 'status-v1.2.3-windows-x64.exe');
    assert.equal(releaseName('status', '1.2.3', 'darwin', 'arm64'), 'status-v1.2.3-macos-arm64');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('rebuilding one platform leaves other platform artifacts intact', async () => {
  const root = await mkdtemp(join(tmpdir(), 'release-'));
  const target = join(root, 'build', 'cargo', 'host');
  try {
    await mkdir(join(target, 'release'), { recursive: true });
    await writeFile(
      join(root, 'package.json'),
      JSON.stringify({ name: 'status', version: '1.2.3' }),
    );
    await writeFile(join(target, 'release', 'status.exe'), 'windows');
    const windows = await stageExecutable(root, target, 'win32', 'x64');
    await writeFile(join(target, 'release', 'status'), 'linux');
    await stageExecutable(root, target, 'linux', 'x64');
    await clearArtifact(root, 'linux', 'x64');
    assert.equal(await readFile(windows.path, 'utf8'), 'windows');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
