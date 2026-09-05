import { randomUUID } from 'node:crypto';
import { open, readFile, rm } from 'node:fs/promises';

const lockError = () => new Error('Another build is active; stop it before starting a new task');

async function readToken(path) {
  return readFile(path, 'utf8').catch((error) => {
    if (error.code === 'ENOENT') return null;
    throw error;
  });
}

export async function acquireBuildLock(path, inherited = process.env.CODEX_STATUS_BUILD_LOCK) {
  if (inherited) {
    if ((await readToken(path)) !== inherited) throw lockError();
    return async () => {
      if ((await readToken(path)) === inherited) await rm(path, { force: true });
    };
  }

  const token = `${process.platform}:${process.pid}:${randomUUID()}`;
  const handle = await open(path, 'wx').catch(() => {
    throw lockError();
  });
  try {
    await handle.writeFile(token);
  } catch (error) {
    await handle.close();
    await rm(path, { force: true });
    throw error;
  }

  return async () => {
    await handle.close();
    if ((await readToken(path)) === token) await rm(path, { force: true });
  };
}
