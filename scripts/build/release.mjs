import { chmod, copyFile, mkdir, readFile, stat } from 'node:fs/promises';
import { basename, dirname, join } from 'node:path';

export function executableName(name, platform = process.platform) {
  return `${name}${platform === 'win32' ? '.exe' : ''}`;
}

export function artifactName(name, platform = process.platform, arch = process.arch) {
  return `${name}-${platform}-${arch}${platform === 'win32' ? '.exe' : ''}`;
}

export async function packageName(root) {
  const manifest = JSON.parse(await readFile(join(root, 'package.json'), 'utf8'));
  if (typeof manifest.name !== 'string' || !manifest.name)
    throw new Error('Package name is missing');
  return manifest.name;
}

export async function stageExecutable(
  root,
  target,
  platform = process.platform,
  arch = process.arch,
) {
  const name = await packageName(root);
  const source = join(target, 'release', executableName(name, platform));
  const sourceInfo = await stat(source);
  if (!sourceInfo.isFile()) throw new Error('Release executable is missing');
  const destination = join(root, 'build', 'release', artifactName(name, platform, arch));
  await mkdir(dirname(destination), { recursive: true });
  await copyFile(source, destination);
  await chmod(destination, sourceInfo.mode);
  const outputInfo = await stat(destination);
  return { path: destination, name: basename(destination), bytes: outputInfo.size };
}
