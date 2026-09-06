import { root } from './clean.mjs';
import { startPackage } from './process.mjs';

function run(args) {
  return startPackage(args, {
    cwd: root,
    group: false,
    stdio: 'inherit',
  });
}

const port = Number(process.argv[2]);
if (!Number.isSafeInteger(port) || port < 1024 || port > 65_535) {
  throw new Error('A valid test server port is required');
}

await run(['exec', 'vite', 'build']).done;
const server = run(['exec', 'vite', 'preview', '--host', '127.0.0.1', '--port', String(port)]);
const stop = () => void server.stop();
process.once('SIGINT', stop);
process.once('SIGTERM', stop);
try {
  await server.done;
} finally {
  await server.stop(true);
}
