// Copyright (C) Gear Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// Local-only, two-validator activation/rotation rehearsal. Requires a fast-runtime local chain,
// indexing from genesis, Alice/Bob validator nodes, and `npm ci --prefix scripts`.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const require = createRequire(import.meta.url);
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { Keyring } = require('@polkadot/keyring');
const { cryptoWaitReady, keccakAsU8a } = require('@polkadot/util-crypto');
const { hexToU8a, u8aToHex, u8aConcat, compactFromU8a } = require('@polkadot/util');
const { secp256k1 } = require('@noble/curves/secp256k1');
const urls = process.argv.slice(2);
assert.equal(urls.length, 2, 'Usage: node scripts/beefy-activation-smoke.mjs ws://127.0.0.1:9944 ws://127.0.0.1:9945');
for (const url of urls) {
  const parsed = new URL(url);
  assert(['ws:', 'wss:'].includes(parsed.protocol) && ['127.0.0.1', '[::1]'].includes(parsed.hostname) && !parsed.username && !parsed.password, 'Never run this test against a public node; use a numeric loopback address');
}
await cryptoWaitReady();
const providers = urls.map(url => new WsProvider(url));
const startupTimeout = setTimeout(() => { console.error('Timed out connecting to local nodes'); process.exit(1); }, 120_000);
const apis = await Promise.all(providers.map(provider => ApiPromise.create({ provider })));
clearTimeout(startupTimeout);
const api = apis[0];
const keyring = new Keyring({ type: 'sr25519' });
const sudoSigner = keyring.addFromUri('//Alice');
const accounts = ['Alice', 'Bob'].map((name, nodeIndex) => ({ name, nodeIndex, controller: keyring.addFromUri('//' + name), stash: keyring.addFromUri('//' + name + '//stash') }));
let proofSubscription;
let proofWork = Promise.resolve();
const proofs = [];
const sessions = [];

async function submit(tx, signer) {
  await new Promise((resolve, reject) => {
    let unsub, settled = false;
    function finish(error) {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      unsub?.();
      if (error) reject(error); else resolve();
    }
    const timer = setTimeout(() => finish(new Error('Timed out finalizing local transaction')), 120_000);
    tx.signAndSend(signer, result => {
      if (result.dispatchError) { finish(new Error(result.dispatchError.toString())); return; }
      for (const { event } of result.events) {
        if (event.section === 'sudo' && ['Sudid', 'SudoAsDone'].includes(event.method) && event.data[0].isErr) {
          finish(new Error('Sudo dispatch failed: ' + event.data[0].asErr)); return;
        }
      }
      if (result.status.isFinalized) finish();
    }).then(value => { unsub = value; if (settled) unsub(); }).catch(finish);
  });
}

async function waitFinalized(label, predicate) {
  await new Promise((resolve, reject) => {
    let unsub, busy = false, settled = false;
    function finish(error) {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      unsub?.();
      if (error) reject(error); else resolve();
    }
    const timer = setTimeout(() => finish(new Error('Timed out: ' + label)), 120_000);
    api.rpc.chain.subscribeFinalizedHeads(async header => {
      if (busy || settled) return;
      busy = true;
      try {
        if (await predicate(await api.at(header.hash), header)) finish();
      } catch (error) { finish(error); }
      finally { busy = false; }
    }).then(value => { unsub = value; if (settled) unsub(); }).catch(finish);
  });
}

function decodeProof(hex) {
  const bytes = hexToU8a(hex);
  let offset = 0;
  function take(length) { assert(offset + length <= bytes.length, 'Truncated BEEFY proof'); const value = bytes.subarray(offset, offset + length); offset += length; return value; }
  function compact() { const [size, number] = compactFromU8a(bytes.subarray(offset)); offset += size; return number.toNumber(); }
  function vector() { return take(compact()); }
  assert.equal(take(1)[0], 1, 'Unsupported BEEFY proof version');
  const start = offset;
  const payloads = [];
  for (let count = compact(); count > 0; count--) payloads.push([Buffer.from(take(2)).toString(), u8aToHex(vector())]);
  const block = Buffer.from(take(4)).readUInt32LE();
  const setId = Buffer.from(take(8)).readBigUInt64LE();
  const commitment = bytes.subarray(start, offset);
  const bitmap = vector();
  const validators = Buffer.from(take(4)).readUInt32LE();
  const signatures = [];
  for (let count = compact(); count > 0; count--) signatures.push(take(65));
  assert.equal(offset, bytes.length, 'Trailing BEEFY proof bytes');
  return { block, setId, commitment, payloads, bitmap, validators, signatures };
}

async function verifyProof(hex) {
  const proof = decodeProof(hex);
  const hash = await api.rpc.chain.getBlockHash(proof.block);
  const at = await api.at(hash);
  const authorities = (await at.query.beefy.authorities()).map(key => key.toU8a());
  assert.equal((await at.query.beefy.validatorSetId()).toBigInt(), proof.setId);
  assert.equal(authorities.length, proof.validators);
  assert.equal(proof.payloads.find(([id]) => id === 'mh')?.[1], (await at.query.mmr.rootHash()).toHex());
  let signatureIndex = 0;
  const digest = keccakAsU8a(proof.commitment, 256);
  for (let index = 0; index < authorities.length; index++) {
    if (!(proof.bitmap[index >> 3] & (1 << (7 - (index & 7))))) continue;
    const signature = proof.signatures[signatureIndex++];
    assert(signature && secp256k1.verify(signature.subarray(0, 64), digest, authorities[index], { lowS: false }), 'Invalid native BEEFY signature');
    const recovered = secp256k1.Signature.fromCompact(signature.subarray(0, 64))
      .addRecoveryBit(signature[64]).recoverPublicKey(digest).toRawBytes(true);
    assert.equal(u8aToHex(recovered), u8aToHex(authorities[index]), 'Invalid native BEEFY recovery byte');
  }
  assert.equal(signatureIndex, proof.signatures.length);
  assert(signatureIndex >= authorities.length - Math.floor((authorities.length - 1) / 3), 'No BEEFY quorum');
  proofs.push({ block: proof.block, setId: proof.setId.toString(), authorities: authorities.map(key => u8aToHex(key)), signatures: signatureIndex });
}

async function register(generation) {
  const registeredAtSession = (await api.query.session.currentIndex()).toNumber();
  const keys = [];
  for (let index = 0; index < accounts.length; index++) {
    const { controller, stash } = accounts[index];
    // Public, valueless local fixtures only. Never use operator secrets in this smoke.
    const secret = new Uint8Array(32).fill(generation + index);
    const stdin = Buffer.from(u8aToHex(secret));
    try {
      const current = (await api.query.session.nextKeys(stash.publicKey)).unwrap();
      const helper = spawnSync(process.execPath, [fileURLToPath(new URL('beefy-session-proof.mjs', import.meta.url)),
        '--genesis', api.genesisHash.toHex(), '--controller', controller.address, '--session-keys', current.toHex()],
      { input: stdin, encoding: 'utf8', timeout: 120_000 });
      assert.equal(helper.status, 0, 'Offline session proof helper failed');
      const proof = JSON.parse(helper.stdout);
      assert.equal(proof.sourceGenesis, api.genesisHash.toHex());
      assert.equal(proof.signer, u8aToHex(controller.publicKey));
      assert.equal(proof.sessionKeys.slice(0, 258), current.toHex().slice(0, 258), 'Helper changed legacy keys');
      assert.equal(proof.sessionKeys.slice(258), proof.beefyPublic.slice(2));
      // Typed Text decodes a 0x seed as UTF-8; raw RPC must preserve the SURI string.
      await providers[accounts[index].nodeIndex].send(
        'author_insertKey', ['beef', stdin.toString(), proof.beefyPublic],
      );
      await submit(api.tx.session.setKeys(proof.sessionKeys, proof.proof), controller);
      keys.push(proof.beefyPublic);
    } finally { secret.fill(0); stdin.fill(0); }
  }
  await waitFinalized('both operational ECDSA keys current and queued', async at => {
    const current = (await at.query.beefy.authorities()).map(key => key.toHex());
    const queued = (await at.query.beefy.nextAuthorities()).map(key => key.toHex());
    return JSON.stringify(current) === JSON.stringify(keys) && JSON.stringify(queued) === JSON.stringify(keys);
  });
  sessions.push({ generation, registeredAtSession, currentAndQueuedAtSession: (await api.query.session.currentIndex()).toNumber(), keys });
  return keys;
}

async function ownershipProof(atHash, name, context, key) {
  const input = u8aToHex(u8aConcat(api.registry.createType('u64', context).toU8a(), key.toU8a()));
  const encoded = await providers[0].send('state_call', [name + 'Api_generate_key_ownership_proof', input, atHash.toHex()]);
  // Decode SCALE explicitly: the decorated opaque return can wrap 0x00 as Some(Bytes).
  return api.registry.createType('Option<Bytes>', hexToU8a(encoded));
}

try {
  for (const node of apis) {
    assert.equal(node.genesisHash.toHex(), api.genesisHash.toHex());
    assert(/local|development/i.test((await node.rpc.system.chain()).toString()), 'Only a disposable local/development chain is allowed');
  }
  const validators = (await api.query.session.validators()).map(value => value.toHex());
  assert.deepEqual([...validators].sort(), accounts.map(value => u8aToHex(value.stash.publicKey)).sort(), 'Expected Alice/Bob local validators');
  accounts.sort((a, b) => validators.indexOf(u8aToHex(a.stash.publicKey)) - validators.indexOf(u8aToHex(b.stash.publicKey)));
  for (const account of accounts) {
    const bonded = (await api.query.staking.bonded(account.stash.publicKey)).unwrap().toHex();
    account.controller = [account.controller, account.stash].find(pair => u8aToHex(pair.publicKey) === bonded);
    assert(account.controller, 'Unknown local bonded signer');
    if (account.controller === account.stash) {
      // Fresh genesis stashes have their whole initial endowment bonded; fund fees locally.
      await submit(api.tx.balances.transferKeepAlive(account.controller.address, 1_000_000_000_000_000n), sudoSigner);
    }
  }
  assert((await api.query.beefy.genesisBlock()).isNone, 'Smoke requires an inactive fresh BEEFY instance');
  const source = api.genesisHash.toHex();
  const chain = '0x' + '0'.repeat(62) + '01';
  const queue = '0x' + '03'.repeat(20);
  await submit(api.tx.sudo.sudo(api.tx.gearEthBridge.bindDestination(chain, queue)), sudoSigner);
  const expected = u8aToHex(keccakAsU8a(u8aConcat(new TextEncoder().encode('vara/gear-eth-bridge-domain/v2'), api.genesisHash.toU8a(), hexToU8a(chain), hexToU8a(queue)), 256));
  assert.equal((await api.query.gearEthBridge.bridgeDomain()).toHex(), expected);
  const firstKeys = await register(11);
  proofSubscription = await providers[0].subscribe('beefy_justifications', 'beefy_subscribeJustifications', [], (error, value) => {
    proofWork = proofWork.then(() => { if (error) throw error; return verifyProof(value); });
    proofWork.catch(() => {});
  });
  await submit(api.tx.sudo.sudo(api.tx.beefy.setNewGenesis(2)), sudoSigner);
  const genesis = (await api.query.beefy.genesisBlock()).unwrap().toNumber();
  await waitFinalized('native quorum commitment after activation', async () => { await proofWork; return proofs.some(proof => proof.block >= genesis && JSON.stringify(proof.authorities) === JSON.stringify(firstKeys)); });
  const secondKeys = await register(21);
  await waitFinalized('native quorum commitment after ECDSA-only rotation', async () => { await proofWork; return proofs.some(proof => JSON.stringify(proof.authorities) === JSON.stringify(secondKeys)); });
  assert.equal((await api.query.gearEthBridge.bridgeDomain()).toHex(), expected);
  const atHash = await api.rpc.chain.getFinalizedHead();
  const at = await api.at(atHash);
  const keys = (await at.query.session.nextKeys(accounts[0].stash.address)).unwrap();
  const grandpaSet = await at.query.grandpa.currentSetId();
  const beefySet = await at.query.beefy.validatorSetId();
  assert((await ownershipProof(atHash, 'Babe', await at.query.babe.currentSlot(), keys.babe)).isSome, 'BABE ownership API proof missing');
  assert((await ownershipProof(atHash, 'Grandpa', grandpaSet, keys.grandpa)).isSome, 'GRANDPA ownership API proof missing');
  assert((await ownershipProof(atHash, 'Beefy', beefySet, keys.beefy)).isSome, 'BEEFY ownership API proof missing');
  assert((await ownershipProof(atHash, 'Grandpa', grandpaSet.toBigInt() + 1n, keys.grandpa)).isNone, 'GRANDPA accepted wrong set ID');
  assert((await ownershipProof(atHash, 'Beefy', beefySet.toBigInt() + 1n, keys.beefy)).isNone, 'BEEFY accepted wrong set ID');
  const purged = accounts.at(-1);
  const purgedKeys = (await api.query.session.nextKeys(purged.stash.publicKey)).unwrap();
  const purgeSession = (await api.query.session.currentIndex()).toNumber();
  await submit(api.tx.session.purgeKeys(), purged.controller);
  const remaining = secondKeys.slice(0, -1);
  await waitFinalized('purged last validator absent from current and queued authority sets', async state => {
    const validators = (await state.query.session.validators()).map(owner => owner.toHex());
    const current = (await state.query.beefy.authorities()).map(key => key.toHex());
    const queued = (await state.query.beefy.nextAuthorities()).map(key => key.toHex());
    const grandpa = await state.call.grandpaApi.grandpaAuthorities();
    const babe = await state.call.babeApi.currentEpoch();
    return !validators.includes(u8aToHex(purged.stash.publicKey))
      && JSON.stringify(current) === JSON.stringify(remaining)
      && JSON.stringify(queued) === JSON.stringify(remaining)
      && !grandpa.some(([key]) => key.eq(purgedKeys.grandpa))
      && !babe.authorities.some(([key]) => key.eq(purgedKeys.babe));
  });
  await waitFinalized('native quorum commitment after validator purge', async () => { await proofWork; return proofs.some(proof => JSON.stringify(proof.authorities) === JSON.stringify(remaining)); });
  const afterPurgeHash = await api.rpc.chain.getFinalizedHead();
  const afterPurge = await api.at(afterPurgeHash);
  assert((await afterPurge.query.session.nextKeys(purged.stash.publicKey)).isNone, 'Purged registration remains');
  assert((await ownershipProof(afterPurgeHash, 'Babe', await afterPurge.query.babe.currentSlot(), purgedKeys.babe)).isNone, 'Purged BABE owner proof remains');
  assert((await ownershipProof(afterPurgeHash, 'Grandpa', await afterPurge.query.grandpa.currentSetId(), purgedKeys.grandpa)).isNone, 'Purged GRANDPA owner proof remains');
  assert((await ownershipProof(afterPurgeHash, 'Beefy', await afterPurge.query.beefy.validatorSetId(), purgedKeys.beefy)).isNone, 'Purged BEEFY owner proof remains');
  assert.equal((await afterPurge.query.gearEthBridge.bridgeDomain()).toHex(), expected);
  sessions.push({ phase: 'purge', purged: purged.stash.address, submittedAtSession: purgeSession, currentAndQueuedAtSession: (await afterPurge.query.session.currentIndex()).toNumber() });
  console.log(JSON.stringify({ sourceGenesis: source, destinationChain: chain, destinationQueue: queue, bridgeDomain: expected, beefyGenesis: genesis, sessionTransitions: sessions, signedCommitments: proofs, ownershipApiProofs: ['BABE', 'GRANDPA', 'BEEFY'], wrongSetIdsRejected: ['GRANDPA', 'BEEFY'], purgedOwnerProofsRejected: ['BABE', 'GRANDPA', 'BEEFY'] }, null, 2));
} finally {
  if (proofSubscription !== undefined) await providers[0].unsubscribe('beefy_justifications', 'beefy_unsubscribeJustifications', proofSubscription);
  await Promise.all(apis.map(node => node.disconnect()));
}
