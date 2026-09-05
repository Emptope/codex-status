import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';

const run = promisify(execFile);
const executable = process.argv[2] || 'codex';
const { stdout } = await run(executable, ['--version'], { timeout: 5000 });
const version = stdout.match(/\b\d+\.\d+\.\d+\b/)?.[0];
if (!version) throw new Error('unknown-version');
const temporary = await mkdtemp(join(tmpdir(), 'status-schema-'));
const destination = new URL(`../../docs/contracts/${version}/`, import.meta.url);
const files = [
  'v1/InitializeParams.json',
  'v1/InitializeResponse.json',
  'v2/GetAccountParams.json',
  'v2/GetAccountResponse.json',
  'v2/GetAccountRateLimitsResponse.json',
  'v2/ThreadListParams.json',
  'v2/ThreadReadParams.json',
  'v2/ThreadStatusChangedNotification.json',
  'v2/ThreadTokenUsageUpdatedNotification.json',
];
try {
  await run(executable, ['app-server', 'generate-json-schema', '--out', temporary], {
    timeout: 30000,
  });
  const hashes = {};
  for (const file of files) {
    const bytes = await readFile(join(temporary, file));
    JSON.parse(bytes.toString('utf8'));
    const output = new URL(file, destination);
    await mkdir(new URL('.', output), { recursive: true });
    await writeFile(output, bytes);
    hashes[file] = createHash('sha256').update(bytes).digest('hex');
  }
  await writeFile(
    new URL('manifest.json', destination),
    JSON.stringify({ version, files: hashes }, null, 2) + '\n',
  );
  console.log(`Exported ${files.length} contracts for ${version}`);
} finally {
  await rm(temporary, { recursive: true, force: true });
}
