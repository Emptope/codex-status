import assert from 'node:assert/strict';
import { PassThrough } from 'node:stream';
import { EventEmitter } from 'node:events';
import test from 'node:test';
import { Rpc, request } from './rpc.mjs';

function processStub() {
  const child = new EventEmitter();
  child.stdin = new PassThrough();
  child.stdout = new PassThrough();
  child.stderr = new PassThrough();
  child.kill = () => {
    queueMicrotask(() => child.emit('close', 0));
    return true;
  };
  return child;
}

test('only observation requests are available and parameters cannot widen access', () => {
  for (const method of ['thread/resume', 'turn/start', 'account/login/start', 'command/exec']) {
    assert.throws(() => request(method, {}), /forbidden/);
  }
  assert.deepEqual(request('account/read', { refreshToken: true }), { refreshToken: false });
  assert.deepEqual(request('thread/list', { useStateDbOnly: false }), {
    limit: 20,
    sortKey: 'updated_at',
    useStateDbOnly: true,
  });
  assert.deepEqual(request('thread/read', { threadId: 'opaque', includeTurns: true }), {
    threadId: 'opaque',
    includeTurns: false,
  });
});

test('partial responses are framed and unsolicited requests never get a reply', async () => {
  const child = processStub();
  const rpc = new Rpc(child, { timeoutMs: 100 });
  const sent = [];
  child.stdin.on('data', (bytes) => sent.push(JSON.parse(bytes.toString())));
  const result = rpc.call('account/read');
  child.stdout.write('{"id":1,"result":');
  child.stdout.write('{"account":null}}\n');
  assert.deepEqual(await result, { account: null });
  child.stdout.write('{"id":88,"method":"item/commandExecution/requestApproval","params":{}}\n');
  assert.equal(sent.length, 1);
  assert.equal(rpc.serverRequests, 1);
  await rpc.close();
});

test('timeouts and process exit reject outstanding queries', async () => {
  const child = processStub();
  const rpc = new Rpc(child, { timeoutMs: 15 });
  await assert.rejects(rpc.call('account/read'), /timeout/);
  const next = rpc.call('account/read');
  child.emit('close', 1);
  await assert.rejects(next, /exited/);
  await rpc.close();
});

test('protocol failures and oversized lines do not expose upstream contents', async () => {
  const child = processStub();
  const rpc = new Rpc(child, { timeoutMs: 100, maxLineBytes: 128 });
  const result = rpc.call('account/read');
  child.stdout.write('{"id":1,"error":{"code":401,"message":"private-credential"}}\n');
  await assert.rejects(result, (error) => error.message === 'rpc-error' && error.code === 401);
  const oversized = rpc.call('account/read');
  child.stdout.write('x'.repeat(129));
  await assert.rejects(oversized, /line-too-large/);
  await rpc.close();
});
