import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { releaseName } from './release.mjs';
import {
  target,
  targets,
  validatePresence,
  validateHost,
  validateVersions,
  verifyArtifact,
  verifyArtifacts,
} from './validate.mjs';

const metadata = { name: 'status', version: '1.2.3' };

async function writeArtifact(root, platform, arch, content = `${platform}-${arch}`) {
  const name = releaseName(metadata.name, metadata.version, platform, arch);
  const directory = join(root, 'build', 'artifacts');
  const path = join(directory, name);
  await mkdir(directory, { recursive: true });
  await writeFile(path, content, { mode: 0o755 });
  const { createHash } = await import('node:crypto');
  const digest = createHash('sha256').update(content).digest('hex');
  await writeFile(`${path}.sha256`, `${digest}  ${name}\n`);
  return path;
}

test('release targets cover the supported systems and architectures', () => {
  assert.deepEqual(
    targets.map(({ platform, arch }) => `${platform}-${arch}`),
    ['win32-x64', 'linux-x64', 'darwin-x64', 'darwin-arm64'],
  );
  assert.equal(target('darwin', 'arm64').label, 'macos');
  assert.throws(() => target('linux', 'arm64'), /Unsupported release target/);
  assert.throws(
    () => validateHost('win32', 'x64', { platform: 'linux', arch: 'x64' }),
    /Runner mismatch/,
  );
});

test('project versions and package names must agree', () => {
  assert.deepEqual(
    validateVersions({ name: 'status' }, {}, { name: 'status', version: '1.2.3' }),
    metadata,
  );
  assert.throws(
    () =>
      validateVersions(
        { name: 'status', version: '1.2.3' },
        { version: '1.2.4' },
        { name: 'status', version: '1.2.3' },
      ),
    /Version mismatch/,
  );
  assert.throws(
    () => validateVersions({ name: 'status' }, {}, { name: 'status', version: 'next' }),
    /semantic versions/,
  );
});

test('desktop windows stay out of the taskbar', () => {
  assert.doesNotThrow(() =>
    validatePresence({
      app: {
        macOSPrivateApi: true,
        windows: [
          { skipTaskbar: true, transparent: true, shadow: false },
          { skipTaskbar: true, transparent: true, shadow: false },
        ],
      },
    }),
  );
  assert.throws(() => validatePresence({ app: { windows: [] } }), /desktop window/);
  assert.throws(
    () =>
      validatePresence({
        app: {
          macOSPrivateApi: true,
          windows: [{ skipTaskbar: true, transparent: true, shadow: false }, {}],
        },
      }),
    /skipTaskbar/,
  );
  assert.throws(
    () =>
      validatePresence({
        app: { macOSPrivateApi: true, windows: [{ skipTaskbar: true, shadow: false }] },
      }),
    /transparency/,
  );
  assert.throws(
    () =>
      validatePresence({
        app: { macOSPrivateApi: true, windows: [{ skipTaskbar: true, transparent: true }] },
      }),
    /native frame shadow/,
  );
  assert.throws(
    () =>
      validatePresence({
        app: { windows: [{ skipTaskbar: true, transparent: true, shadow: false }] },
      }),
    /macOS private API/,
  );
});

test('artifact validation checks names, hashes, and the complete release set', async () => {
  const root = await mkdtemp(join(tmpdir(), 'release-check-'));
  try {
    for (const { platform, arch } of targets) await writeArtifact(root, platform, arch);
    const linux = await verifyArtifact(root, metadata, 'linux', 'x64');
    assert.equal(linux.name, 'status-v1.2.3-linux-x64.deb');
    assert.equal((await verifyArtifacts(root, metadata)).length, 4);

    await writeFile(linux.checksumPath, `${'0'.repeat(64)}  ${linux.name}\n`);
    await assert.rejects(verifyArtifact(root, metadata, 'linux', 'x64'), /Checksum mismatch/);
    await writeFile(join(root, 'build', 'artifacts', 'unexpected'), 'extra');
    await assert.rejects(verifyArtifacts(root, metadata), /Release set mismatch/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('GitHub releases publish only validated artifacts and checksums', async () => {
  const workflow = await readFile(
    new URL('../../.github/workflows/release.yml', import.meta.url),
    'utf8',
  );
  const command = workflow.split('\n').find((line) => line.includes('gh release create'));
  assert.match(command, /build\/artifacts\/\*/);
  assert.doesNotMatch(command, /\bLICENSE\b/);
});
