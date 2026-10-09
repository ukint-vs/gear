// Copyright (C) Gear Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
function run(script, argv) {
  return spawnSync(process.execPath, [fileURLToPath(new URL(script, import.meta.url)), ...argv],
    { encoding: 'utf8', timeout: 10_000 });
}

test('live smoke rejects public URLs and ambiguous hosts before opening RPC', () => {
  for (const url of ['wss://rpc.vara.network', 'ws://example.com', 'ws://localhost:9944',
    'ws://127.0.0.1.example.com', 'http://127.0.0.1:9944', 'ws://user:password@127.0.0.1:9944']) {
    const result = run('beefy-activation-smoke.mjs', [url, 'ws://127.0.0.1:9945']);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Never run this test against a public node/);
    assert.equal(result.stdout, '');
  }
});
