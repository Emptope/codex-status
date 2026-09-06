import { createHash } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { basename, dirname, join } from 'node:path';

export function executableName(name, platform = process.platform) {
  return `${name}${platform === 'win32' ? '.exe' : ''}`;
}

export function platformName(platform) {
  if (platform === 'win32') return 'windows';
  if (platform === 'darwin') return 'macos';
  return platform;
}

function bundleType(platform) {
  if (platform === 'win32') return null;
  if (platform === 'linux') return 'deb';
  if (platform === 'darwin') return 'dmg';
  throw new Error(`Unsupported release platform: ${platform}`);
}

export function bundleArgs(platform = process.platform) {
  const bundle = bundleType(platform);
  return bundle ? ['--bundles', bundle, '--ci'] : ['--no-bundle', '--ci'];
}

function releaseStem(name, version, platform, arch) {
  return `${name}-v${version}-${platformName(platform)}-${arch}`;
}

export function releaseName(name, version, platform = process.platform, arch = process.arch) {
  const bundle = bundleType(platform);
  const extension = bundle ? `.${bundle}` : '.exe';
  return `${releaseStem(name, version, platform, arch)}${extension}`;
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

async function releaseSource(target, name, platform) {
  const bundle = bundleType(platform);
  if (!bundle) return join(target, 'release', executableName(name, platform));
  const directory = join(target, 'release', 'bundle', bundle);
  const entries = await readdir(directory, { withFileTypes: true });
  const packages = entries.filter((entry) => entry.isFile() && entry.name.endsWith(`.${bundle}`));
  if (packages.length !== 1) throw new Error(`Expected one ${bundle} release package`);
  return join(directory, packages[0].name);
}

export async function stageArtifact(
  root,
  target,
  metadata,
  platform = process.platform,
  arch = process.arch,
) {
  const source = await releaseSource(target, metadata.name, platform);
  const sourceInfo = await stat(source);
  if (!sourceInfo.isFile()) throw new Error('Release artifact is missing');
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

export async function clearArtifact(
  root,
  metadata,
  platform = process.platform,
  arch = process.arch,
) {
  const directory = join(root, 'build', 'artifacts');
  const stem = releaseStem(metadata.name, metadata.version, platform, arch);
  const entries = await readdir(directory).catch((error) => {
    if (error.code === 'ENOENT') return [];
    throw error;
  });
  const paths = entries
    .filter((name) => name === stem || name.startsWith(`${stem}.`))
    .map((name) => join(directory, name));
  await Promise.all(paths.map((path) => rm(path, { force: true })));
}
