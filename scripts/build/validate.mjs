import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { appendFile, lstat, readFile, readdir } from 'node:fs/promises';
import { basename, join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { buildLayout, inBuild } from './layout.mjs';
import { releaseName } from './release.mjs';

const execute = promisify(execFile);
const root = fileURLToPath(new URL('../../', import.meta.url));
const semver =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;

export const targets = Object.freeze([
  Object.freeze({ platform: 'win32', label: 'windows', arch: 'x64' }),
  Object.freeze({ platform: 'linux', label: 'linux', arch: 'x64' }),
  Object.freeze({ platform: 'darwin', label: 'macos', arch: 'x64' }),
  Object.freeze({ platform: 'darwin', label: 'macos', arch: 'arm64' }),
]);

export function target(platform, arch) {
  const value = targets.find((item) => item.platform === platform && item.arch === arch);
  if (!value) throw new Error(`Unsupported release target: ${platform}-${arch}`);
  return value;
}

export function validateHost(platform, arch, actual = process) {
  target(platform, arch);
  if (actual.platform !== platform || actual.arch !== arch) {
    throw new Error(
      `Runner mismatch: expected ${platform}-${arch}, got ${actual.platform}-${actual.arch}`,
    );
  }
}

export function validateVersions(packageManifest, tauriConfig, cargoPackage) {
  const versions = [cargoPackage.version, packageManifest.version, tauriConfig.version].filter(
    (version) => version !== undefined,
  );
  if (versions.some((version) => typeof version !== 'string' || !semver.test(version))) {
    throw new Error('Project versions must be valid semantic versions');
  }
  if (new Set(versions).size !== 1) {
    throw new Error(
      `Version mismatch: cargo=${cargoPackage.version}, package=${packageManifest.version}, tauri=${tauriConfig.version}`,
    );
  }
  if (packageManifest.name !== cargoPackage.name) {
    throw new Error(
      `Package name mismatch: package=${packageManifest.name}, cargo=${cargoPackage.name}`,
    );
  }
  return { name: packageManifest.name, version: cargoPackage.version };
}

export function validatePresence(tauriConfig) {
  const windows = tauriConfig.app?.windows;
  if (!Array.isArray(windows) || windows.length === 0) {
    throw new Error('Project must configure at least one desktop window');
  }
  if (windows.some((window) => window?.skipTaskbar !== true)) {
    throw new Error('Every desktop window must enable skipTaskbar');
  }
  if (windows.some((window) => window?.transparent !== true)) {
    throw new Error('Every desktop window must enable transparency for rounded cards');
  }
  if (windows.some((window) => window?.shadow !== false)) {
    throw new Error('Every desktop window must disable the native frame shadow');
  }
  if (tauriConfig.app?.macOSPrivateApi !== true) {
    throw new Error('macOS private API must be enabled for transparent rounded cards');
  }
}

export async function projectMetadata(directory = root) {
  const [packageSource, tauriSource, cargo] = await Promise.all([
    readFile(join(directory, 'package.json'), 'utf8'),
    readFile(join(directory, 'src-tauri', 'tauri.conf.json'), 'utf8'),
    execute(
      'cargo',
      [
        'metadata',
        '--manifest-path',
        join(directory, 'src-tauri', 'Cargo.toml'),
        '--format-version',
        '1',
        '--no-deps',
        '--locked',
      ],
      { cwd: directory, maxBuffer: 4 * 1024 * 1024 },
    ),
  ]);
  const packageManifest = JSON.parse(packageSource);
  const tauriConfig = JSON.parse(tauriSource);
  const cargoMetadata = JSON.parse(cargo.stdout);
  const members = new Set(cargoMetadata.workspace_members);
  const cargoPackages = cargoMetadata.packages.filter((item) => members.has(item.id));
  if (cargoPackages.length !== 1) throw new Error('Expected one Cargo workspace package');
  validatePresence(tauriConfig);
  return validateVersions(packageManifest, tauriConfig, cargoPackages[0]);
}

function artifactNames(metadata) {
  return targets.flatMap(({ platform, arch }) => {
    const name = releaseName(metadata.name, metadata.version, platform, arch);
    return [name, `${name}.sha256`];
  });
}

export async function verifyArtifact(directory, metadata, platform, arch) {
  target(platform, arch);
  const name = releaseName(metadata.name, metadata.version, platform, arch);
  const path = join(inBuild(directory, buildLayout.artifacts), name);
  const checksumPath = `${path}.sha256`;
  const [info, content, checksum] = await Promise.all([
    lstat(path),
    readFile(path),
    readFile(checksumPath, 'utf8'),
  ]);
  if (!info.isFile() || info.isSymbolicLink() || info.size === 0) {
    throw new Error(`Invalid release artifact: ${name}`);
  }
  const digest = createHash('sha256').update(content).digest('hex');
  const expected = `${digest}  ${basename(path)}\n`;
  if (checksum !== expected) throw new Error(`Checksum mismatch: ${name}`);
  return { name, path, checksumPath, digest };
}

export async function verifyArtifacts(directory, metadata) {
  const artifactDirectory = inBuild(directory, buildLayout.artifacts);
  const expected = artifactNames(metadata).sort();
  const actual = (await readdir(artifactDirectory)).sort();
  if (actual.length !== expected.length || actual.some((name, index) => name !== expected[index])) {
    throw new Error(
      `Release set mismatch: expected ${expected.join(', ')}, got ${actual.join(', ')}`,
    );
  }
  return Promise.all(
    targets.map(({ platform, arch }) => verifyArtifact(directory, metadata, platform, arch)),
  );
}

async function output(name, value) {
  if (process.env.GITHUB_OUTPUT) await appendFile(process.env.GITHUB_OUTPUT, `${name}=${value}\n`);
}

async function main([command, ...args]) {
  if (command === 'host') {
    validateHost(args[0], args[1]);
    console.log(`Validated runner ${args[0]}-${args[1]}`);
    return;
  }
  const metadata = await projectMetadata(root);
  if (command === 'metadata') {
    console.log(`Validated ${metadata.name} v${metadata.version}`);
    return;
  }
  if (command === 'version') {
    console.log(metadata.version);
    return;
  }
  if (command === 'tag') {
    const expected = `v${metadata.version}`;
    if (args[0] !== expected) throw new Error(`Tag must be ${expected}`);
    console.log(`Validated tag ${expected}`);
    return;
  }
  if (command === 'artifact') {
    validateHost(args[0], args[1]);
    const artifact = await verifyArtifact(root, metadata, args[0], args[1]);
    await output('name', artifact.name);
    console.log(`Validated ${artifact.name}`);
    return;
  }
  if (command === 'artifacts') {
    const artifacts = await verifyArtifacts(root, metadata);
    console.log(`Validated ${artifacts.length} release artifacts`);
    return;
  }
  throw new Error('Expected metadata, version, tag, host, artifact, or artifacts');
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  await main(process.argv.slice(2));
}
