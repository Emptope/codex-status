import { lstat, mkdir, readdir, realpath, rm } from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { buildLayout, inBuild } from './layout.mjs';

export const root = fileURLToPath(new URL('../../', import.meta.url));

async function canonicalDirectory(directory, message) {
  const info = await lstat(directory).catch((error) => {
    if (error.code === 'ENOENT') return null;
    throw error;
  });
  if (!info) return null;
  if (info.isSymbolicLink() || !info.isDirectory()) {
    throw new Error(message);
  }
  const canonical = join(await realpath(dirname(directory)), basename(directory));
  if ((await realpath(directory)) !== canonical) throw new Error(message);
  return canonical;
}

async function removeDirectory(directory, message) {
  const canonical = await canonicalDirectory(directory, message);
  if (canonical) await rm(canonical, { recursive: true, force: true });
}

async function prepareBuildDirectory(directory) {
  const canonical = await canonicalDirectory(directory, 'Unsafe build directory');
  if (canonical) return canonical;
  await mkdir(directory, { recursive: true });
  return canonicalDirectory(directory, 'Unsafe build directory');
}

async function artifactFilesOnly(directory) {
  const entries = await readdir(directory, { withFileTypes: true }).catch((error) => {
    if (error.code === 'ENOENT') return [];
    throw error;
  });
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) await removeDirectory(path, 'Unsafe artifact directory');
    else if (!entry.isFile()) throw new Error('Unsafe artifact output');
  }
}

export async function cleanOutputs(directory, { preserveArtifacts = false } = {}) {
  const requested = directory;
  directory = await prepareBuildDirectory(directory);
  for (const name of [
    'staging',
    'test',
    'preview',
    'web',
    'release',
    'screenshots',
    'test-results',
  ]) {
    await removeDirectory(join(directory, name), 'Unsafe build output');
  }
  const artifacts = join(directory, 'artifacts');
  if (preserveArtifacts) await artifactFilesOnly(artifacts);
  else await removeDirectory(artifacts, 'Unsafe build output');
  for (const name of ['cargo', 'vite-cache']) {
    await removeDirectory(join(directory, name), 'Unsafe legacy build output');
  }
  const cargo = join(directory, 'cache', 'cargo');
  const targets = await readdir(cargo, { withFileTypes: true }).catch((error) => {
    if (error.code === 'ENOENT') return [];
    throw error;
  });
  for (const target of targets) {
    if (target.isDirectory()) {
      await removeDirectory(join(cargo, target.name, '.tauri'), 'Unsafe bundle cache');
      await removeDirectory(join(cargo, target.name, 'release', 'bundle'), 'Unsafe bundle output');
    }
  }
  return requested;
}

export async function cleanCaches(directory) {
  const requested = directory;
  directory = await prepareBuildDirectory(directory);
  await removeDirectory(join(directory, 'cache'), 'Unsafe build cache');
  for (const name of ['cargo', 'vite-cache']) {
    await removeDirectory(join(directory, name), 'Unsafe legacy build cache');
  }
  return requested;
}

export async function cleanBuild(options) {
  const repository = await realpath(root);
  return cleanOutputs(inBuild(repository, buildLayout.root), options);
}

export async function cleanBuildCaches() {
  const repository = await realpath(root);
  return cleanCaches(inBuild(repository, buildLayout.root));
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const mode = process.argv[2] || 'outputs';
  if (process.argv.length > 3 || !['outputs', 'cache'].includes(mode)) {
    throw new Error('Expected outputs or cache; build path overrides are forbidden');
  }
  if (mode === 'cache') await cleanBuildCaches();
  else await cleanBuild();
}
