import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm, stat, utimes, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { cargoTarget, runWithNormalizedTimes, stabilizeFingerprintTimes } from './cargo.mjs';

test('isolates Cargo targets by host platform and architecture', () => {
  assert.equal(cargoTarget('/repo', 'linux', 'x64'), join('/repo', 'build', 'cargo', 'linux-x64'));
  assert.equal(cargoTarget('/repo', 'win32', 'x64'), join('/repo', 'build', 'cargo', 'win32-x64'));
});

test('stabilizes only fingerprints updated by the current Cargo task', async () => {
  const target = await mkdtemp(join(tmpdir(), 'cargo-times-'));
  const old = join(target, 'debug', '.fingerprint', 'old-hash', 'invoked.timestamp');
  const current = join(target, 'debug', '.fingerprint', 'current-hash', 'invoked.timestamp');
  const generated = join(target, 'debug', 'build', 'crate-hash', 'out', 'generated.rs');
  const recent = new Date('2026-01-02T03:04:05.678Z');
  const stable = new Date('2026-01-02T03:04:07.000Z');
  try {
    for (const file of [old, generated]) {
      await mkdir(dirname(file), { recursive: true });
      await writeFile(file, 'data');
      await utimes(file, recent, recent);
    }
    const before = new Map([
      [
        old,
        `${(await stat(old, { bigint: true })).mtimeNs}:${(await stat(old, { bigint: true })).ctimeNs}`,
      ],
    ]);
    await mkdir(dirname(current), { recursive: true });
    await writeFile(current, 'data');

    await stabilizeFingerprintTimes(target, before, stable);

    assert.equal((await stat(old)).mtimeMs, recent.getTime());
    assert.equal((await stat(current)).mtimeMs, stable.getTime());
    assert.equal((await stat(generated)).mtimeMs, recent.getTime());
  } finally {
    await rm(target, { recursive: true, force: true });
  }
});

test('stabilizes Cargo fingerprints after a failed build', async () => {
  const target = await mkdtemp(join(tmpdir(), 'cargo-failure-times-'));
  const invoked = join(target, 'release', '.fingerprint', 'crate-hash', 'invoked.timestamp');
  try {
    await mkdir(dirname(invoked), { recursive: true });
    await assert.rejects(
      runWithNormalizedTimes(target, async () => {
        await writeFile(invoked, '');
        throw new Error('build-failed');
      }),
      /build-failed/,
    );
    assert.equal((await stat(invoked)).mtimeMs % 1000, 0);
  } finally {
    await rm(target, { recursive: true, force: true });
  }
});
