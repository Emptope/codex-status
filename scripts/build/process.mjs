import { spawn } from 'node:child_process';

async function terminateWindows(pid, force) {
  const args = ['/PID', String(pid), '/T'];
  if (force) args.push('/F');
  const child = spawn('taskkill', args, {
    stdio: 'ignore',
    windowsHide: true,
  });
  await new Promise((resolve) => {
    child.once('error', resolve);
    child.once('exit', resolve);
  });
}

async function descendants(parent) {
  const child = spawn('ps', ['-A', '-o', 'pid=,ppid='], {
    stdio: ['ignore', 'pipe', 'ignore'],
  });
  let output = '';
  child.stdout.on('data', (chunk) => {
    if (output.length < 1024 * 1024) output += chunk;
  });
  await new Promise((resolve) => {
    child.once('error', resolve);
    child.once('exit', resolve);
  });
  const children = new Map();
  for (const line of output.split('\n')) {
    const [pid, ppid] = line.trim().split(/\s+/).map(Number);
    if (!Number.isSafeInteger(pid) || !Number.isSafeInteger(ppid)) continue;
    const values = children.get(ppid) || [];
    values.push(pid);
    children.set(ppid, values);
  }
  const result = [];
  const visit = (pid) => {
    for (const child of children.get(pid) || []) {
      visit(child);
      result.push(child);
    }
  };
  visit(parent);
  return result;
}

async function terminateUnix(child, force, tracked, grouped) {
  const signal = force ? 'SIGKILL' : 'SIGTERM';
  if (!settled(child)) {
    for (const pid of await descendants(child.pid)) tracked.add(pid);
  }
  for (const pid of tracked) {
    try {
      process.kill(pid, signal);
    } catch (error) {
      if (error.code !== 'ESRCH') throw error;
    }
  }
  if (grouped) {
    try {
      process.kill(-child.pid, signal);
    } catch (error) {
      if (error.code !== 'ESRCH' && !settled(child)) child.kill(signal);
    }
  } else if (!settled(child)) {
    child.kill(signal);
  }
}

const settled = (child) => child.exitCode !== null || child.signalCode !== null;

export function start(command, args, options = {}) {
  const windows = process.platform === 'win32';
  const { group = true, shell = false, ...spawnOptions } = options;
  const child = spawn(command, args, {
    ...spawnOptions,
    detached: group && !windows,
    shell,
    windowsHide: true,
  });
  const tracked = new Set();
  let finished = false;
  let stopping = false;
  let stopChain = Promise.resolve();
  const done = new Promise((resolve, reject) => {
    child.once('error', (error) => {
      finished = true;
      if (stopping) resolve();
      else reject(error);
    });
    child.once('exit', (code) => {
      finished = true;
      if (code === 0 || stopping) resolve();
      else reject(new Error(`${command} exited ${code}`));
    });
  });

  return {
    child,
    done,
    stop(force = false) {
      stopping = true;
      const previous = stopChain.catch(() => {});
      stopChain = previous.then(async () => {
        if ((!force && finished) || !child.pid) return;
        if (windows) {
          if (!finished) await terminateWindows(child.pid, force);
          return;
        }
        await terminateUnix(child, force, tracked, group);
      });
      return stopChain;
    },
  };
}

export function startPackage(args, options = {}) {
  const entry = process.env.npm_execpath;
  if (!entry) throw new Error('Package manager execution path is unavailable');
  return start(process.execPath, [entry, ...args], options);
}
