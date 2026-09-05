import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { setTimeout as delay } from 'node:timers/promises';
import { start } from './process.mjs';

async function waitForExit(pid) {
  let state = null;
  for (let attempt = 0; attempt < 40; attempt++) {
    try {
      process.kill(pid, 0);
      if (process.platform === 'linux') {
        state = await readFile(`/proc/${pid}/stat`, 'utf8')
          .then((value) =>
            value
              .slice(value.lastIndexOf(')') + 2)
              .split(' ')
              .slice(0, 2)
              .join(':'),
          )
          .catch(() => null);
        if (state === null || state.startsWith('Z:')) return;
      }
    } catch (error) {
      if (error.code === 'ESRCH') return;
      throw error;
    }
    await delay(25);
  }
  throw new Error(`process-still-running:${state}`);
}

test('stopping a job terminates its process tree without reporting a failure', async () => {
  const source = String.raw`
    const { spawn } = require('node:child_process');
    const child = spawn(process.execPath, ['-e', 'process.on("SIGTERM", () => {}); console.log("ready"); setInterval(() => {}, 1000)'], {
      stdio: ['ignore', 'pipe', 'ignore']
    });
    child.stdout.once('data', () => console.log(child.pid));
    setInterval(() => {}, 1000);
  `;
  const job = start(process.execPath, ['-e', source], {
    shell: false,
    stdio: ['ignore', 'pipe', 'ignore'],
  });
  const chunks = [];
  job.child.stdout.on('data', (chunk) => chunks.push(chunk));
  for (let attempt = 0; attempt < 40 && !chunks.join('').includes('\n'); attempt++) {
    await delay(25);
  }
  const descendant = Number(chunks.join('').trim());
  assert.ok(Number.isSafeInteger(descendant));

  const graceful = job.stop();
  job.child.kill('SIGTERM');
  await job.done;
  await job.stop(true);
  await graceful;
  await waitForExit(descendant);
});

test('an unexpected non-zero exit remains a failure', async () => {
  const job = start(process.execPath, ['-e', 'process.exit(7)'], {
    shell: false,
    stdio: 'ignore',
  });
  await assert.rejects(job.done, /exited 7/);
});
