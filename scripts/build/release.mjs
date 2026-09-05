import { createHash } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { basename, dirname, join } from 'node:path';

export function executableName(name, platform = process.platform) {
  return `${name}${platform === 'win32' ? '.exe' : ''}`;
}

export function platformName(platform) {
  if (platform === 'win32') return 'windows';
  if (platform === 'darwin') return 'macos';
  return platform;
}

export function releaseName(name, version, platform = process.platform, arch = process.arch) {
  const extension = platform === 'win32' ? '.exe' : '';
  return `${name}-v${version}-${platformName(platform)}-${arch}${extension}`;
}

export function artifactPath(
  root,
  name,
  version,
  platform = process.platform,
  arch = process.arch,
) {
  return join(root, 'build', 'artifacts', releaseName(name, version, platform, arch));
}

async function packageManifest(root) {
  return JSON.parse(await readFile(join(root, 'package.json'), 'utf8'));
}

async function packageMetadata(root) {
  const manifest = await packageManifest(root);
  if (typeof manifest.name !== 'string' || !manifest.name)
    throw new Error('Package name is missing');
  if (typeof manifest.version !== 'string' || !manifest.version)
    throw new Error('Package version is missing');
  return { name: manifest.name, version: manifest.version };
}

export async function stageExecutable(
  root,
  target,
  platform = process.platform,
  arch = process.arch,
) {
  const metadata = await packageMetadata(root);
  const source = join(target, 'release', executableName(metadata.name, platform));
  const sourceInfo = await stat(source);
  if (!sourceInfo.isFile()) throw new Error('Release executable is missing');
  const destination = artifactPath(root, metadata.name, metadata.version, platform, arch);
  await mkdir(dirname(destination), { recursive: true });
  await copyFile(source, destination);
  await chmod(destination, sourceInfo.mode);
  const outputInfo = await stat(destination);
  const digest = createHash('sha256')
    .update(await readFile(destination))
    .digest('hex');
  const checksumPath = `${destination}.sha256`;
  await writeFile(checksumPath, `${digest}  ${basename(destination)}\n`);
  return {
    path: destination,
    checksumPath,
    name: basename(destination),
    bytes: outputInfo.size,
  };
}

export async function clearArtifact(root, platform = process.platform, arch = process.arch) {
  const metadata = await packageMetadata(root);
  const path = artifactPath(root, metadata.name, metadata.version, platform, arch);
  await Promise.all([rm(path, { force: true }), rm(`${path}.sha256`, { force: true })]);
}
