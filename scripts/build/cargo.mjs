import { readdir, stat, utimes } from 'node:fs/promises';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export function cargoTarget(root, platform = process.platform, arch = process.arch) {
  return join(root, 'build', 'cargo', `${platform}-${arch}`);
}

async function directories(path) {
  const entries = await readdir(path, { withFileTypes: true }).catch((error) => {
    if (error.code === 'ENOENT') return [];
    throw error;
  });
  return entries.filter((entry) => entry.isDirectory()).map((entry) => join(path, entry.name));
}

async function invocationFiles(target) {
  const files = [];
  for (const first of await directories(target)) {
    for (const profile of [first, ...(await directories(first))]) {
      for (const unit of await directories(join(profile, '.fingerprint'))) {
        const path = join(unit, 'invoked.timestamp');
        const exists = await stat(path)
          .then((info) => info.isFile())
          .catch((error) => {
            if (error.code === 'ENOENT') return false;
            throw error;
          });
        if (exists) files.push(path);
      }
    }
  }
  return files;
}

async function fingerprintState(target) {
  const state = new Map();
  await Promise.all(
    (await invocationFiles(target)).map(async (path) => {
      const info = await stat(path, { bigint: true });
      state.set(path, `${info.mtimeNs}:${info.ctimeNs}`);
    }),
  );
  return state;
}

async function nextSecond() {
  const boundary = Math.floor(Date.now() / 1000) * 1000 + 1000;
  await delay(boundary - Date.now() + 25);
  return new Date(boundary);
}

export async function stabilizeFingerprintTimes(target, before, time) {
  const changed = [];
  for (const path of await invocationFiles(target)) {
    const info = await stat(path, { bigint: true });
    if (before.get(path) !== `${info.mtimeNs}:${info.ctimeNs}`) changed.push(path);
  }
  if (changed.length === 0) return;
  const stableTime = time || (await nextSecond());
  await Promise.all(changed.map((path) => utimes(path, stableTime, stableTime)));
}

export async function runWithNormalizedTimes(target, task) {
  const before = await fingerprintState(target);
  try {
    return await task();
  } finally {
    await stabilizeFingerprintTimes(target, before);
  }
}
