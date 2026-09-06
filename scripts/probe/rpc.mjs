import { spawn } from 'node:child_process';
import manifest from '../../package.json' with { type: 'json' };

export function request(method, params = {}) {
  switch (method) {
    case 'initialize':
      return { clientInfo: { name: 'codex_status_probe', version: manifest.version || '0.0.0' } };
    case 'account/read':
      return { refreshToken: false };
    case 'account/rateLimits/read':
      return {};
    case 'thread/list':
      return { limit: 20, sortKey: 'updated_at', useStateDbOnly: true };
    case 'thread/read':
      if (typeof params.threadId !== 'string' || !params.threadId || params.threadId.length > 256) {
        throw new Error('invalid-thread-id');
      }
      return { threadId: params.threadId, includeTurns: false };
    default:
      throw new Error('forbidden-method');
  }
}

export class Rpc {
  #child;
  #pending = new Map();
  #sequence = 0;
  #buffer = Buffer.alloc(0);
  #timeoutMs;
  #maxLineBytes;
  #closed = false;
  #exited;
  methods = [];
  serverRequests = 0;
  stderrBytes = 0;

  constructor(child, { timeoutMs = 15000, maxLineBytes = 4 * 1024 * 1024 } = {}) {
    this.#child = child;
    this.#timeoutMs = timeoutMs;
    this.#maxLineBytes = maxLineBytes;
    this.#exited = new Promise((resolve) => {
      child.once('close', () => {
        this.#closed = true;
        this.#fail('process-exited');
        resolve();
      });
    });
    child.on('error', () => this.#fail('process-error'));
    child.stdin.on('error', () => this.#fail('transport-error'));
    child.stderr.on('data', (bytes) => {
      this.stderrBytes += bytes.length;
    });
    child.stdout.on('data', (bytes) => this.#receive(bytes));
  }

  static start(executable = 'codex', env = process.env) {
    return new Rpc(
      spawn(executable, ['app-server', '--listen', 'stdio://'], {
        env,
        shell: false,
        windowsHide: true,
        stdio: ['pipe', 'pipe', 'pipe'],
      }),
    );
  }

  #fail(reason) {
    for (const pending of this.#pending.values()) pending.reject(new Error(reason));
    this.#pending.clear();
  }

  #receive(bytes) {
    this.#buffer = Buffer.concat([this.#buffer, bytes]);
    let end;
    while ((end = this.#buffer.indexOf(10)) !== -1) {
      if (end > this.#maxLineBytes) return this.#invalid();
      const line = this.#buffer.subarray(0, end);
      this.#buffer = this.#buffer.subarray(end + 1);
      let message;
      try {
        message = JSON.parse(line.toString('utf8'));
      } catch {
        this.#fail('invalid-json');
        continue;
      }
      if (!message || typeof message !== 'object') continue;
      if (message.method) {
        if (message.id !== undefined) this.serverRequests++;
        continue;
      }
      const pending = this.#pending.get(message.id);
      if (!pending) continue;
      this.#pending.delete(message.id);
      if (message.error) {
        const error = new Error('rpc-error');
        error.code = Number.isSafeInteger(message.error.code) ? message.error.code : null;
        pending.reject(error);
      } else if (Object.hasOwn(message, 'result')) pending.resolve(message.result);
      else pending.reject(new Error('invalid-response'));
    }
    if (this.#buffer.length > this.#maxLineBytes) this.#invalid();
  }

  #invalid() {
    this.#buffer = Buffer.alloc(0);
    this.#fail('line-too-large');
    this.#child.kill('SIGTERM');
  }

  call(method, params) {
    const safeParams = request(method, params);
    if (this.#closed) return Promise.reject(new Error('process-exited'));
    const id = ++this.#sequence;
    this.methods.push(method);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#pending.delete(id);
        reject(new Error('request-timeout'));
      }, this.#timeoutMs);
      this.#pending.set(id, {
        resolve: (value) => {
          clearTimeout(timer);
          resolve(value);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
      });
      this.#child.stdin.write(JSON.stringify({ id, method, params: safeParams }) + '\n');
    });
  }

  async initialize() {
    await this.call('initialize');
    this.methods.push('initialized');
    this.#child.stdin.write(JSON.stringify({ method: 'initialized', params: {} }) + '\n');
  }

  async close() {
    if (this.#closed) return;
    this.#fail('connection-closed');
    this.#child.stdin.end();
    this.#child.kill('SIGTERM');
    const timer = setTimeout(() => this.#child.kill('SIGKILL'), 1000);
    await this.#exited;
    clearTimeout(timer);
  }
}
