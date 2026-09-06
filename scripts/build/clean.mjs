import { lstat, mkdir, readdir, realpath, rm } from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

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
  directory = await canonicalDirectory(directory, 'Unsafe build directory');
  if (!directory) {
    await mkdir(requested, { recursive: true });
    directory = await canonicalDirectory(requested, 'Unsafe build directory');
  }
  for (const name of ['web', 'release', 'screenshots', 'test-results']) {
    await removeDirectory(join(directory, name), 'Unsafe build output');
  }
  const artifacts = join(directory, 'artifacts');
  if (preserveArtifacts) await artifactFilesOnly(artifacts);
  else await removeDirectory(artifacts, 'Unsafe build output');
  const cargo = join(directory, 'cargo');
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

export async function cleanBuild(options) {
  const repository = await realpath(root);
  return cleanOutputs(join(repository, 'build'), options);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.argv.length !== 2) throw new Error('Build path overrides are forbidden');
  await cleanBuild();
}
