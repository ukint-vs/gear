// Copyright (C) Gear Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
const require = createRequire(import.meta.url);
const { secp256k1 } = require('@noble/curves/secp256k1');
const { keccakAsU8a, encodeAddress } = require('@polkadot/util-crypto');
const { hexToU8a, u8aToHex, u8aConcat } = require('@polkadot/util');
const genesis = '0x' + 'ab'.repeat(32);
const signer = '0x' + 'cd'.repeat(32);
const legacy = '11'.repeat(32) + '22'.repeat(32) + '33'.repeat(32) + '44'.repeat(32);
const bundle = '0x' + legacy + '00'.repeat(33);
// Public, valueless fixture; real secrets must enter the helper on stdin only.
const fixture = '01'.repeat(32);
const args = ['--genesis', genesis, '--controller', signer, '--session-keys', bundle];
function run(script, argv, input = '') {
  return spawnSync(process.execPath, [fileURLToPath(new URL(script, import.meta.url)), ...argv],
    { input, encoding: 'utf8', timeout: 10_000 });
}
function digest(source, owner, keys) {
  return keccakAsU8a(u8aConcat(new TextEncoder().encode('vara/beefy-session-keys/v1'),
    hexToU8a(source), hexToU8a(owner), hexToU8a(keys)), 256);
}

test('offline proof binds real genesis, actual signer and every key in the 161-byte output', () => {
  const result = run('beefy-session-proof.mjs', args, '0x' + fixture + '\n');
  assert.equal(result.status, 0, result.stderr);
  const proof = JSON.parse(result.stdout);
  const publicKey = u8aToHex(secp256k1.getPublicKey(hexToU8a('0x' + fixture), true));
  assert.equal(proof.sourceGenesis, genesis);
  assert.equal(proof.signer, signer);
  assert.equal(proof.beefyPublic, publicKey);
  assert.equal(proof.sessionKeys, '0x' + legacy + publicKey.slice(2));
  assert.equal(hexToU8a(proof.sessionKeys).length, 161);
  const signature = hexToU8a(proof.proof);
  assert.equal(signature.length, 65);
  const payload = digest(genesis, signer, proof.sessionKeys);
  assert.equal(proof.payload, u8aToHex(payload));
  assert(secp256k1.verify(signature.subarray(0, 64), payload, hexToU8a(publicKey), { lowS: true }));
  assert.equal(u8aToHex(secp256k1.Signature.fromCompact(signature.subarray(0, 64))
    .addRecoveryBit(signature[64]).recoverPublicKey(payload).toRawBytes(true)), publicKey);
  for (const offset of [0, 32, 64, 96, 128, 160]) {
    const changed = hexToU8a(proof.sessionKeys);
    changed[offset] ^= 1;
    assert(!secp256k1.verify(signature.subarray(0, 64), digest(genesis, signer, u8aToHex(changed)), hexToU8a(publicKey)));
  }
  for (const [source, owner] of [['0x' + 'ef'.repeat(32), signer], [genesis, '0x' + 'ef'.repeat(32)]]) {
    assert(!secp256k1.verify(signature.subarray(0, 64), digest(source, owner, proof.sessionKeys), hexToU8a(publicKey)));
  }
  const ss58 = [...args];
  ss58[3] = encodeAddress(hexToU8a(signer), 137);
  assert.deepEqual(JSON.parse(run('beefy-session-proof.mjs', ss58, fixture).stdout), proof);
  assert(!(result.stdout + result.stderr).includes(fixture), 'Secret fixture leaked');
});

test('helper rejects malformed public inputs and secrets without printing stdin', () => {
  for (const [index, value] of [[1, '0x' + '00'.repeat(32)], [1, '0x12'], [3, 'not-an-account'], [5, '0x' + legacy]]) {
    const badArgs = [...args];
    badArgs[index] = value;
    const result = run('beefy-session-proof.mjs', badArgs, fixture);
    assert.notEqual(result.status, 0);
    assert.equal(result.stdout, '');
    assert(!(result.stdout + result.stderr).includes(fixture));
  }
  for (const input of ['', fixture + fixture, '0x' + 'gg'.repeat(32), '0x' + '00'.repeat(32)]) {
    const result = run('beefy-session-proof.mjs', args, input);
    assert.notEqual(result.status, 0);
    assert.equal(result.stdout, '');
    if (input) assert(!(result.stdout + result.stderr).includes(input), 'Invalid secret leaked');
  }
  const extra = run('beefy-session-proof.mjs', [...args, fixture], fixture);
  assert.notEqual(extra.status, 0);
  assert(!(extra.stdout + extra.stderr).includes(fixture));
});

test('live smoke rejects public URLs and ambiguous hosts before opening RPC', () => {
  for (const url of ['wss://rpc.vara.network', 'ws://example.com', 'ws://localhost:9944',
    'ws://127.0.0.1.example.com', 'http://127.0.0.1:9944', 'ws://user:password@127.0.0.1:9944']) {
    const result = run('beefy-activation-smoke.mjs', [url, 'ws://127.0.0.1:9945']);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Never run this test against a public node/);
    assert.equal(result.stdout, '');
  }
});
