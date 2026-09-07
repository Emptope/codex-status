import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { cargoTarget, runWithNormalizedTimes } from './cargo.mjs';
import { cleanBuild, root } from './clean.mjs';
import { acquireBuildLock } from './lock.mjs';
import { buildLayout, inBuild } from './layout.mjs';
import { normalizeColorEnv, start, startPackage } from './process.mjs';
import { bundleArgs, stageArtifact } from './release.mjs';
import { projectMetadata, verifyArtifact } from './validate.mjs';

const task = process.argv[2];
if (!['verify', 'build', 'dev', 'preview'].includes(task)) throw new Error('Unknown task');
const lockPath = inBuild(root, buildLayout.lock);
await mkdir(inBuild(root, buildLayout.root), { recursive: true });
const releaseLock = await acquireBuildLock(lockPath);
const jobs = new Set();
const env = normalizeColorEnv({ ...process.env, CARGO_TARGET_DIR: cargoTarget(root) });
let interrupted = false;
let stopping;

async function track(job) {
  if (interrupted) throw new Error('task-interrupted');
  jobs.add(job);
  try {
    await job.done;
  } finally {
    jobs.delete(job);
  }
  if (interrupted) throw new Error('task-interrupted');
}

async function run(command, args, taskEnv = env) {
  return track(start(command, args, { cwd: root, env: taskEnv, stdio: 'inherit' }));
}

async function runPackage(args, taskEnv = env) {
  return track(startPackage(args, { cwd: root, env: taskEnv, stdio: 'inherit' }));
}

async function runBuild(command, args, taskEnv = env) {
  await runWithNormalizedTimes(taskEnv.CARGO_TARGET_DIR, () => run(command, args, taskEnv));
}

async function runPackageBuild(args, taskEnv = env) {
  await runWithNormalizedTimes(taskEnv.CARGO_TARGET_DIR, () => runPackage(args, taskEnv));
}

async function ready(url, process) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const response = await fetch(url).catch(() => null);
    if (response?.ok) return;
    await Promise.race([
      delay(100),
      process.then(() => {
        throw new Error('Development server exited before becoming ready');
      }),
    ]);
  }
  throw new Error('Development server did not become ready');
}

async function stop() {
  if (stopping) return stopping;
  stopping = (async () => {
    const active = [...jobs];
    await Promise.all(active.map((job) => job.stop()));
    await Promise.race([Promise.allSettled(active.map((job) => job.done)), delay(3000)]);
    await Promise.all(active.map((job) => job.stop(true)));
    await Promise.allSettled(active.map((job) => job.done));
  })();
  return stopping;
}

function interrupt() {
  interrupted = true;
  void stop();
}
process.once('SIGINT', interrupt);
process.once('SIGTERM', interrupt);
let failure;
try {
  await cleanBuild({ preserveArtifacts: task !== 'build' });
  const metadata = await projectMetadata(root);
  if (task === 'verify') {
    await runPackage(['run', 'lint:actions']);
    await runPackage([
      'exec',
      'prettier',
      '--check',
      '.github',
      'src',
      'scripts',
      'tests',
      'package.json',
      'vite.config.ts',
      'vitest.config.ts',
      'playwright.config.ts',
      'svelte.config.js',
      'tsconfig.json',
    ]);
    await runPackage(['exec', 'svelte-check', '--tsconfig', 'tsconfig.json']);
    await run('cargo', ['fmt', '--manifest-path', 'src-tauri/Cargo.toml', '--check']);
    await runBuild('cargo', [
      'clippy',
      '--manifest-path',
      'src-tauri/Cargo.toml',
      '--profile',
      'test',
      '--all-targets',
      '--locked',
      '--',
      '-D',
      'warnings',
    ]);
    await runBuild('cargo', ['test', '--manifest-path', 'src-tauri/Cargo.toml', '--locked']);
    await run('node', [
      '--test',
      '--experimental-test-isolation=none',
      'scripts/build/cargo.test.mjs',
      'scripts/build/clean.test.mjs',
      'scripts/build/lock.test.mjs',
      'scripts/build/process.test.mjs',
      'scripts/build/release.test.mjs',
      'scripts/build/validate.test.mjs',
      'scripts/probe/records.test.mjs',
      'scripts/probe/rpc.test.mjs',
    ]);
    await runPackage(['exec', 'vitest', 'run']);
    await runPackage(['exec', 'playwright', 'test']);
  } else if (task === 'build') {
    await runPackage(['exec', 'vite', 'build']);
    await runPackageBuild(['exec', 'tauri', 'build', ...bundleArgs()]);
    const artifact = await stageArtifact(root, env.CARGO_TARGET_DIR, metadata);
    await verifyArtifact(root, metadata, process.platform, process.arch);
    console.log(`Built release artifact at ${artifact.path} (${artifact.bytes} bytes)`);
    console.log(`Wrote checksum at ${artifact.checksumPath}`);
  } else if (task === 'dev') {
    const vite = runPackage(['exec', 'vite']);
    vite.catch(() => {});
    await ready('http://127.0.0.1:1420', vite);
    await runPackageBuild(['exec', 'tauri', 'dev']);
  } else {
    await mkdir(inBuild(root, buildLayout.preview), { recursive: true });
    await runBuild('cargo', [
      'build',
      '--manifest-path',
      'src-tauri/Cargo.toml',
      '--no-default-features',
      '--locked',
    ]);
    env.CODEX_STATUS_PREVIEW = '1';
    await runPackage(['exec', 'vite', '--host', '127.0.0.1']);
  }
} catch (error) {
  if (!interrupted) failure = error;
} finally {
  await stop();
  await releaseLock();
}
if (failure) throw failure;
if (interrupted && !['dev', 'preview'].includes(task)) process.exitCode = 130;
