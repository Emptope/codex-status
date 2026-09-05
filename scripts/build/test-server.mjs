import { root } from './clean.mjs';
import { startPackage } from './process.mjs';

function run(args) {
  return startPackage(args, {
    cwd: root,
    group: false,
    stdio: 'inherit',
  });
}

await run(['exec', 'vite', 'build']).done;
const server = run(['exec', 'vite', 'preview', '--host', '127.0.0.1', '--port', '1420']);
const stop = () => void server.stop();
process.once('SIGINT', stop);
process.once('SIGTERM', stop);
try {
  await server.done;
} finally {
  await server.stop(true);
}
