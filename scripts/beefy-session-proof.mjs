// Copyright (C) Gear Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// Offline only: takes a 32-byte ECDSA secret (hex, optional 0x) on protected stdin.
// Public arguments are the real genesis hash, actual bonded signer, and encoded SessionKeys.
// Never pass a private key in argv, an environment variable, or an RPC signing request.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { decodeAddress, keccakAsU8a, cryptoWaitReady } = require('@polkadot/util-crypto');
const { hexToU8a, u8aToHex, u8aConcat } = require('@polkadot/util');
const { secp256k1 } = require('@noble/curves/secp256k1');
const args = process.argv.slice(2);
assert.equal(args.length, 6, 'Usage: node scripts/beefy-session-proof.mjs --genesis 0x... --controller ADDRESS --session-keys 0x... < /protected/ecdsa-seed');
assert(args[0] === '--genesis' && args[2] === '--controller' && args[4] === '--session-keys', 'Invalid public arguments');
assert(/^0x[\da-fA-F]{64}$/.test(args[1]) && !/^0x0{64}$/.test(args[1]), 'A real nonzero genesis hash is required');
assert(/^0x[\da-fA-F]{322}$/.test(args[5]), 'SessionKeys must encode the four legacy keys and appended BEEFY key (161 bytes)');
assert(!process.stdin.isTTY, 'Read the ECDSA seed from protected stdin; terminal echo is unsafe');
await cryptoWaitReady();
const source = hexToU8a(args[1]);
const account = decodeAddress(args[3]);
assert.equal(account.length, 32, 'Signer must be an AccountId32');
const input = readFileSync(0);
const secret = new Uint8Array(32);
try {
  let first = 0, last = input.length;
  while (first < last && input[first] <= 32) first++;
  while (last > first && input[last - 1] <= 32) last--;
  if (input[first] === 48 && (input[first + 1] === 120 || input[first + 1] === 88)) first += 2;
  assert.equal(last - first, 64, 'Protected stdin must contain exactly one 32-byte hex ECDSA secret');
  const nibble = byte => byte >= 48 && byte <= 57 ? byte - 48 : byte >= 65 && byte <= 70 ? byte - 55 : byte >= 97 && byte <= 102 ? byte - 87 : -1;
  for (let i = 0; i < 32; i++) {
    const a = nibble(input[first + i * 2]), b = nibble(input[first + i * 2 + 1]);
    assert(a >= 0 && b >= 0, 'Invalid ECDSA secret encoding');
    secret[i] = a * 16 + b;
  }
  const publicKey = secp256k1.getPublicKey(secret, true);
  const keys = u8aConcat(hexToU8a(args[5]).subarray(0, 128), publicKey);
  const payload = keccakAsU8a(u8aConcat(new TextEncoder().encode('vara/beefy-session-keys/v1'), source, account, keys), 256);
  const signed = secp256k1.sign(payload, secret, { lowS: true });
  const signature = u8aConcat(signed.toCompactRawBytes(), new Uint8Array([signed.recovery]));
  assert(secp256k1.verify(signature.subarray(0, 64), payload, publicKey), 'ECDSA proof self-check failed');
  console.log(JSON.stringify({ sourceGenesis: args[1], signer: u8aToHex(account), beefyPublic: u8aToHex(publicKey), sessionKeys: u8aToHex(keys), payload: u8aToHex(payload), proof: u8aToHex(signature) }, null, 2));
} finally {
  secret.fill(0);
  input.fill(0);
}
