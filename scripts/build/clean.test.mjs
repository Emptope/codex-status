import assert from 'node:assert/strict';
import { access, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { cleanOutputs } from './clean.mjs';

test('task cleanup preserves flat artifacts and removes legacy artifact directories', async () => {
  const root = await mkdtemp(join(tmpdir(), 'clean-'));
  const build = join(root, 'build');
  const artifact = join(build, 'artifacts', 'status-v1.2.3-windows-x64.exe');
  const legacy = join(build, 'artifacts', 'windows', 'x64', 'status.exe');
  try {
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
    await rm(root, { recursive: true, force: true });
  }
});
