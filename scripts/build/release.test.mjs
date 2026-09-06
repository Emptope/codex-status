import assert from 'node:assert/strict';
import { access, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import {
  artifactPath,
  bundleArgs,
  clearArtifact,
  executableName,
  releaseName,
  stageArtifact,
} from './release.mjs';

const metadata = { name: 'status', version: '1.2.3' };

test('release artifacts are separated by platform and architecture', () => {
  assert.equal(executableName('status', 'win32'), 'status.exe');
  assert.equal(executableName('status', 'linux'), 'status');
  assert.deepEqual(bundleArgs('win32'), ['--no-bundle', '--ci']);
  assert.deepEqual(bundleArgs('linux'), ['--bundles', 'deb', '--ci']);
  assert.deepEqual(bundleArgs('darwin'), ['--bundles', 'dmg', '--ci']);
  assert.equal(
    artifactPath('/repo', 'status', '1.2.3', 'win32', 'x64'),
    join('/repo', 'build', 'artifacts', 'status-v1.2.3-windows-x64.exe'),
  );
  assert.equal(
    artifactPath('/repo', 'status', '1.2.3', 'linux', 'x64'),
    join('/repo', 'build', 'artifacts', 'status-v1.2.3-linux-x64.deb'),
  );
  assert.equal(
    artifactPath('/repo', 'status', '1.2.3', 'darwin', 'arm64'),
    join('/repo', 'build', 'artifacts', 'status-v1.2.3-macos-arm64.dmg'),
  );
});

test('a release package and checksum are staged in the unified artifact directory', async () => {
  const root = await mkdtemp(join(tmpdir(), 'release-'));
  const target = join(root, 'build', 'cargo', 'host');
  try {
    const bundle = join(target, 'release', 'bundle', 'deb');
    await mkdir(bundle, { recursive: true });
    await writeFile(join(bundle, 'generated.deb'), 'package');
    const artifact = await stageArtifact(root, target, metadata, 'linux', 'x64');
    assert.equal(artifact.name, 'status-v1.2.3-linux-x64.deb');
    assert.equal(artifact.path, join(root, 'build', 'artifacts', 'status-v1.2.3-linux-x64.deb'));
    assert.equal(await readFile(artifact.path, 'utf8'), 'package');
    assert.equal(
      await readFile(artifact.checksumPath, 'utf8'),
      'bc4a71180870f7945155fbb02f4b0a2e3faa2a62d6d31b7039013055ed19869a  status-v1.2.3-linux-x64.deb\n',
    );
    assert.equal(releaseName('status', '1.2.3', 'win32', 'x64'), 'status-v1.2.3-windows-x64.exe');
    assert.equal(
      releaseName('status', '1.2.3', 'darwin', 'arm64'),
      'status-v1.2.3-macos-arm64.dmg',
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('rebuilding one platform leaves other platform artifacts intact', async () => {
  const root = await mkdtemp(join(tmpdir(), 'release-'));
  const target = join(root, 'build', 'cargo', 'host');
  try {
    await mkdir(join(target, 'release'), { recursive: true });
    await writeFile(join(target, 'release', 'status.exe'), 'windows');
    const windows = await stageArtifact(root, target, metadata, 'win32', 'x64');
    const bundle = join(target, 'release', 'bundle', 'deb');
    await mkdir(bundle, { recursive: true });
    await writeFile(join(bundle, 'generated.deb'), 'linux');
    const linux = await stageArtifact(root, target, metadata, 'linux', 'x64');
    const legacy = join(root, 'build', 'artifacts', 'status-v1.2.3-linux-x64');
    await writeFile(legacy, 'legacy');
    await writeFile(`${legacy}.sha256`, 'legacy');
    await clearArtifact(root, metadata, 'linux', 'x64');
    assert.equal(await readFile(windows.path, 'utf8'), 'windows');
    await assert.rejects(access(linux.path));
    await assert.rejects(access(legacy));
    await assert.rejects(access(`${legacy}.sha256`));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
