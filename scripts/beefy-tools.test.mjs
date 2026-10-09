// Copyright (C) Gear Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

test('live smoke rejects public URLs and ambiguous hosts before opening RPC', () => {
  const scriptPath = fileURLToPath(new URL('beefy-activation-smoke.mjs', import.meta.url));
  for (const url of ['wss://rpc.vara.network', 'ws://example.com', 'ws://localhost:9944',
    'ws://127.0.0.1.example.com', 'http://127.0.0.1:9944', 'ws://user:password@127.0.0.1:9944']) {
    const result = spawnSync(process.execPath, [scriptPath, url, 'ws://127.0.0.1:9945'],
      { encoding: 'utf8', timeout: 10_000 });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Never run this test against a public node/);
    assert.equal(result.stdout, '');
  }
});
