import { lstat, mkdir, readdir, realpath, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const root = fileURLToPath(new URL('../../', import.meta.url));

async function removeDirectory(directory, message) {
  const info = await lstat(directory).catch((error) => {
    if (error.code === 'ENOENT') return null;
    throw error;
  });
  if (
    info &&
    (info.isSymbolicLink() || !info.isDirectory() || (await realpath(directory)) !== directory)
  ) {
    throw new Error(message);
  }
  await rm(directory, { recursive: true, force: true });
}

export async function cleanBuild() {
  const repository = await realpath(root);
  const directory = join(repository, 'build');
  const info = await lstat(directory).catch((error) => {
    if (error.code === 'ENOENT') return null;
    throw error;
  });
  if (
    info &&
    (info.isSymbolicLink() || !info.isDirectory() || (await realpath(directory)) !== directory)
  ) {
    throw new Error('Unsafe build directory');
  }
  await mkdir(directory, { recursive: true });
  for (const name of ['web', 'release', 'screenshots', 'test-results']) {
    await removeDirectory(join(directory, name), 'Unsafe build output');
  }
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
  return directory;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.argv.length !== 2) throw new Error('Build path overrides are forbidden');
  await cleanBuild();
}
