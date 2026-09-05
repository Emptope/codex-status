import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { cargoTarget } from './cargo.mjs';
import { root } from './clean.mjs';

export function preview() {
  return {
    name: 'local-status-preview',
    configureServer(server) {
      if (process.env.CODEX_STATUS_PREVIEW !== '1') return;
      let snapshot = {
        revision: 0,
        sessions: [],
        quotas: [],
        connection: 'connecting',
        account: 'unknown',
        provider: null,
        version: null,
        updatedAt: null,
        error: null,
        localError: null,
        refreshing: false,
      };
      const settingsPath = join(root, 'build', 'preview-settings.json');
      const defaults = {
        schema: 1,
        roots: [],
        executable: 'codex',
        theme: 'system',
        fontSize: 13,
        alwaysOnTop: true,
        collapsed: false,
        autoFollow: true,
        pinnedSession: null,
        selectedBucket: null,
        notifications: true,
        muted: false,
        lowQuota: 10,
        position: null,
      };
      const binary = join(
        cargoTarget(root),
        'debug',
        process.platform === 'win32' ? 'codex-status.exe' : 'codex-status',
      );
      const child = spawn(binary, ['--collector', settingsPath], {
        stdio: ['pipe', 'pipe', 'ignore'],
        windowsHide: true,
      });
      let partial = '';
      child.stdout.on('data', (bytes) => {
        partial += bytes.toString('utf8');
        if (partial.length > 4 * 1024 * 1024) {
          partial = '';
          return;
        }
        let end;
        while ((end = partial.indexOf('\n')) !== -1) {
          const line = partial.slice(0, end);
          partial = partial.slice(end + 1);
          try {
            snapshot = JSON.parse(line);
          } catch {}
        }
      });
      child.on('error', () => {
        snapshot = { ...snapshot, connection: 'unavailable' };
      });
      child.stdin.on('error', () => {});
      server.httpServer?.once('close', () => child.stdin.end());
      server.middlewares.use(async (request, response, next) => {
        if (!request.url?.startsWith('/api/')) {
          next();
          return;
        }
        if (request.headers.origin && request.headers.origin !== `http://${request.headers.host}`) {
          response.writeHead(403).end();
          return;
        }
        response.setHeader('Content-Type', 'application/json');
        response.setHeader('Cache-Control', 'no-store');
        const name = request.url.slice(5);
        if (request.method === 'GET' && name === 'snapshot') {
          response.end(JSON.stringify(snapshot));
          return;
        }
        if (request.method === 'GET' && name === 'preferences') {
          const settings = await readFile(settingsPath, 'utf8')
            .then(JSON.parse)
            .catch(() => defaults);
          response.end(JSON.stringify(settings));
          return;
        }
        if (request.method !== 'POST' || !['refresh', 'save_preferences'].includes(name)) {
          response.writeHead(404).end();
          return;
        }
        let body = '';
        for await (const chunk of request) {
          body += chunk;
          if (body.length > 65536) {
            response.writeHead(413).end();
            return;
          }
        }
        try {
          const args = JSON.parse(body);
          child.stdin.write(
            JSON.stringify({
              command: name,
              ...(name === 'save_preferences' ? { settings: args.settings } : {}),
            }) + '\n',
          );
          response.writeHead(204).end();
        } catch {
          response.writeHead(400).end();
        }
      });
    },
  };
}
