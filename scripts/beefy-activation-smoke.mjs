// Copyright (C) Gear Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// Local-only two-phase rehearsal; bootstrap Bob stays up until the first invocation exits.
// Requires fast-runtime, archival/indexed Alice and protected Bob, and CI-only npm dependencies.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { Keyring } = require('@polkadot/keyring');
const { cryptoWaitReady, keccakAsU8a } = require('@polkadot/util-crypto');
const { hexToU8a, u8aToHex, u8aConcat, compactFromU8a } = require('@polkadot/util');
const { secp256k1 } = require('@noble/curves/secp256k1');
const args = process.argv.slice(2);
const afterRestart = args.at(-1) === '--after-restart';
const urls = afterRestart ? args.slice(0, -1) : args;
assert.equal(urls.length, 2, 'Usage: node scripts/beefy-activation-smoke.mjs ws://127.0.0.1:9944 ws://127.0.0.1:9945 [--after-restart]');
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
// These valueless local transaction accounts are the only secrets held by JavaScript.
const sudoSigner = keyring.addFromUri('//Alice');
const accounts = ['Alice', 'Bob'].map((name, nodeIndex) => ({ name, nodeIndex, controller: keyring.addFromUri('//' + name), stash: keyring.addFromUri('//' + name + '//stash') }));
const fields = ['babe', 'grandpa', 'imOnline', 'authorityDiscovery', 'beefy'];
let proofSubscription;
let proofWork = Promise.resolve();
const proofs = [];
const sessions = [];
const mmrProofs = [];
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const beefyKeys = bundles => bundles.map(keys => keys.beefy.toHex());

function dispatchName(error) {
  if (!error.isModule) return error.toString();
  const decoded = api.registry.findMetaError(error.asModule);
  return decoded.section + '.' + decoded.name;
}

async function submit(tx, signer, expectedError) {
  return new Promise((resolve, reject) => {
    let unsub, settled = false;
    function finish(error, result) {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      unsub?.();
      if (error) reject(error); else resolve(result);
    }
    const timer = setTimeout(() => finish(new Error('Timed out finalizing local transaction')), 120_000);
    tx.signAndSend(signer, result => {
      if (!result.status.isFinalized) return;
      try {
        let error = result.dispatchError;
        for (const { event } of result.events) {
          if (event.section === 'sudo' && ['Sudid', 'SudoAsDone'].includes(event.method) && event.data[0].isErr) error = event.data[0].asErr;
        }
        if (expectedError) assert.equal(error && dispatchName(error), expectedError);
        else assert(!error, 'Dispatch failed: ' + (error && dispatchName(error)));
        finish(undefined, result);
      } catch (error) { finish(error); }
    }).then(value => { unsub = value; if (settled) unsub(); }).catch(finish);
  });
}

async function waitFinalized(label, predicate) {
  return new Promise((resolve, reject) => {
    let unsub, busy = false, settled = false;
    function finish(error, value) {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      unsub?.();
      if (error) reject(error); else resolve(value);
    }
    const timer = setTimeout(() => finish(new Error('Timed out: ' + label)), 120_000);
    api.rpc.chain.subscribeFinalizedHeads(async header => {
      if (busy || settled) return;
      busy = true;
      try {
        const value = await predicate(await api.at(header.hash), header);
        if (value) finish(undefined, value);
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
  assert(authorities.length > 0);
  assert.equal(proof.payloads.filter(([id]) => id === 'mh').length, 1);
  const root = (await at.query.mmr.rootHash()).toHex();
  assert.equal(proof.payloads.find(([id]) => id === 'mh')[1], root);
  assert.equal(proof.bitmap.length, Math.floor(authorities.length / 8) + 1);
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
  for (let index = authorities.length; index < proof.bitmap.length * 8; index++) assert.equal(proof.bitmap[index >> 3] & (1 << (7 - (index & 7))), 0);
  assert.equal(signatureIndex, proof.signatures.length);
  // Native threshold requires all signatures for committees of one, two or three.
  assert(signatureIndex >= authorities.length - Math.floor((authorities.length - 1) / 3), 'No BEEFY quorum');
  if (!proofs.some(value => value.block === proof.block && value.setId === proof.setId.toString())) {
    proofs.push({ block: proof.block, blockHash: hash.toHex(), setId: proof.setId.toString(), authorities: authorities.map(key => u8aToHex(key)), signatures: signatureIndex, mmrRoot: root, justification: hex });
  }
}

async function queuedBundles(at) {
  const queued = await at.query.session.queuedKeys();
  assert.deepEqual(queued.map(([owner]) => owner.toHex()), accounts.map(account => u8aToHex(account.stash.publicKey)));
  return queued.map(([, keys]) => keys);
}

async function currentBundlesMatch(at, bundles) {
  const validators = (await at.query.session.validators()).map(owner => owner.toHex());
  if (!same(validators, accounts.map(account => u8aToHex(account.stash.publicKey)))) return false;
  const babe = await at.call.babeApi.currentEpoch();
  const grandpa = await at.call.grandpaApi.grandpaAuthorities();
  const online = await at.query.imOnline.keys();
  const discovery = await at.query.authorityDiscovery.keys();
  const beefy = await at.query.beefy.authorities();
  return same(babe.authorities.map(([key]) => key.toHex()), bundles.map(keys => keys.babe.toHex()))
    && same(grandpa.map(([key]) => key.toHex()), bundles.map(keys => keys.grandpa.toHex()))
    && same(online.map(key => key.toHex()), bundles.map(keys => keys.imOnline.toHex()))
    && same(discovery.map(key => key.toHex()), bundles.map(keys => keys.authorityDiscovery.toHex()))
    && same(beefy.map(key => key.toHex()), beefyKeys(bundles));
}

async function register(phase) {
  const registeredAtSession = (await api.query.session.currentIndex()).toNumber();
  const bundles = [];
  for (const { nodeIndex, controller, stash } of accounts) {
    const owner = u8aToHex(controller.publicKey);
    assert.equal(hexToU8a(owner).length, 32);
    // Native RPC uses seed None and the node's configured persistent keystore/password.
    const generated = await providers[nodeIndex].send('author_rotateKeysWithOwner', [owner]);
    assert(generated && typeof generated.keys === 'string' && typeof generated.proof === 'string', 'Native RPC must return both keys and ownership proof');
    assert.match(generated.keys, /^0x[0-9a-fA-F]{322}$/);
    assert.match(generated.proof, /^0x[0-9a-fA-F]{642}$/);
    const type = api.tx.session.setKeys.meta.args[0].type.toString();
    const keys = api.registry.createType(type, hexToU8a(generated.keys));
    assert.equal(keys.toU8a().length, 161);
    assert.equal(keys.toHex(), generated.keys.toLowerCase());
    assert.deepEqual([...keys.keys()], fields);
    for (const field of fields) assert.equal(keys[field].toU8a().length, field === 'beefy' ? 33 : 32);
    assert.equal(await providers[nodeIndex].send('author_hasSessionKeys', [generated.keys]), true, 'Native bundle missing from local keystore');
    await submit(api.tx.session.setKeys(generated.keys, generated.proof), controller);
    assert.equal((await api.query.session.nextKeys(stash.publicKey)).unwrap().toHex(), keys.toHex());
    bundles.push(keys);
  }
  const queuedAtSession = await waitFinalized('entire native bundles queued', async at => {
    const queued = await queuedBundles(at);
    return same(queued.map(keys => keys.toHex()), bundles.map(keys => keys.toHex())) && { index: (await at.query.session.currentIndex()).toNumber() };
  });
  const currentAtSession = await waitFinalized('all five native fields current and queued', async at => {
    if (!same((await queuedBundles(at)).map(keys => keys.toHex()), bundles.map(keys => keys.toHex()))) return false;
    if (!(await currentBundlesMatch(at, bundles))) return false;
    assert(same((await at.query.beefy.nextAuthorities()).map(key => key.toHex()), beefyKeys(bundles)));
    assert(same((await at.query.authorityDiscovery.nextKeys()).map(key => key.toHex()), bundles.map(keys => keys.authorityDiscovery.toHex())));
    return { index: (await at.query.session.currentIndex()).toNumber() };
  });
  assert(currentAtSession.index > registeredAtSession, 'No session handover observed');
  sessions.push({ phase, registeredAtSession, queuedAtSession: queuedAtSession.index, currentAndQueuedAtSession: currentAtSession.index, keys: bundles.map(keys => keys.toHex()) });
  return bundles;
}

async function ownershipProof(atHash, name, context, key) {
  const input = u8aToHex(u8aConcat(api.registry.createType('u64', context).toU8a(), key.toU8a()));
  const encoded = await providers[0].send('state_call', [name + 'Api_generate_key_ownership_proof', input, atHash.toHex()]);
  // Decode SCALE explicitly: the decorated opaque return can wrap 0x00 as Some(Bytes).
  return api.registry.createType('Option<Bytes>', hexToU8a(encoded));
}

async function checkOwnership(atHash, bundles) {
  const at = await api.at(atHash);
  const slot = await at.query.babe.currentSlot();
  const grandpaSet = await at.query.grandpa.currentSetId();
  const beefySet = await at.query.beefy.validatorSetId();
  for (const keys of bundles) {
    assert((await ownershipProof(atHash, 'Babe', slot, keys.babe)).isSome, 'BABE ownership API proof missing');
    assert((await ownershipProof(atHash, 'Grandpa', grandpaSet, keys.grandpa)).isSome, 'GRANDPA ownership API proof missing');
    assert((await ownershipProof(atHash, 'Beefy', beefySet, keys.beefy)).isSome, 'BEEFY ownership API proof missing');
    assert((await ownershipProof(atHash, 'Grandpa', grandpaSet.toBigInt() + 1n, keys.grandpa)).isNone, 'GRANDPA accepted wrong set ID');
    assert((await ownershipProof(atHash, 'Beefy', beefySet.toBigInt() + 1n, keys.beefy)).isNone, 'BEEFY accepted wrong set ID');
  }
}

async function checkMmr(commitment, domain) {
  const at = await api.at(commitment.blockHash);
  assert.equal((await at.query.mmr.rootHash()).toHex(), commitment.mmrRoot);
  const generated = await api.rpc.mmr.generateProof([commitment.block], commitment.block, commitment.blockHash);
  assert.equal(generated.blockHash.toHex(), commitment.blockHash);
  assert((await api.rpc.mmr.verifyProofStateless(commitment.mmrRoot, generated)).isTrue, 'Historical MMR proof failed');
  const leaves = api.registry.createType('Vec<Bytes>', generated.leaves.toU8a(true));
  assert.equal(leaves.length, 1);
  const leaf = Buffer.from(leaves[0].toU8a(true));
  assert.equal(leaf.length, 113, 'Unexpected five-field BEEFY MMR leaf encoding');
  assert.equal(leaf[0], 0);
  assert.equal(leaf.readUInt32LE(1), commitment.block - 1);
  const header = await api.rpc.chain.getHeader(commitment.blockHash);
  assert.equal(u8aToHex(leaf.subarray(5, 37)), header.parentHash.toHex());
  const next = await at.call.beefyMmrApi.nextAuthoritySetProof();
  assert.equal(u8aToHex(leaf.subarray(37, 81)), next.toHex());
  // Leaf extra commits the parent timestamp/queue and lane domain, not plaintext domain bytes.
  const parent = await api.at(header.parentHash);
  assert.equal((await parent.query.gearEthBridge.bridgeDomain()).toHex(), domain);
  const root = await parent.query.gearEthBridge.queueMerkleRoot();
  const initialized = (await parent.query.gearEthBridge.initialized()).isTrue && root.isSome;
  const snapshot = u8aConcat(Uint8Array.of(2), new TextEncoder().encode('vara'), hexToU8a(domain),
    (await parent.query.timestamp.now()).toU8a(), Uint8Array.of(initialized ? 1 : 0),
    api.registry.createType('u64', initialized ? await parent.query.gearEthBridge.queueId() : 0).toU8a(),
    initialized ? root.unwrap().toU8a() : new Uint8Array(32));
  assert.equal(snapshot.length, 86);
  assert.equal(u8aToHex(leaf.subarray(81)), u8aToHex(keccakAsU8a(snapshot, 256)), 'Signed MMR leaf has wrong bridge snapshot/domain');
  const evidence = { block: commitment.block, blockHash: commitment.blockHash, root: commitment.mmrRoot, bridgeDomain: domain, leaf: u8aToHex(leaf), leaves: generated.leaves.toHex(), proof: generated.proof.toHex() };
  mmrProofs.push(evidence);
  return evidence;
}

async function waitCommitment(label, keys, beyond = -1) {
  return waitFinalized(label, async () => {
    await proofWork;
    return proofs.find(proof => proof.block > beyond && same(proof.authorities, keys));
  });
}

async function bridgeSend(payload, expectedError) {
  await waitFinalized('bridge initialized and session cleanup complete', async at => (await at.query.gearEthBridge.initialized()).isTrue && (await at.query.gearEthBridge.clearTimer()).isNone);
  const result = await submit(api.tx.gearEthBridge.sendEthMessage('0x' + '08'.repeat(20), payload), sudoSigner, expectedError);
  const events = result.events.map(({ event }) => event);
  if (!expectedError) assert(events.some(event => event.section === 'gearEthBridge' && event.method === 'MessageQueued'), 'Successful bridge send did not queue a message');
  else {
    assert(!events.some(event => event.section === 'gearEthBridge' && event.method === 'MessageQueued'));
    assert(!events.some(event => event.section === 'balances' && event.method === 'Transfer'), 'Paused send transferred bridge fee');
    const header = await api.rpc.chain.getHeader(result.status.asFinalized);
    const before = await api.at(header.parentHash);
    const after = await api.at(header.hash);
    assert.equal((await after.query.gearEthBridge.messageNonce()).toHex(), (await before.query.gearEthBridge.messageNonce()).toHex(), 'Paused send changed message nonce');
    // A session hook may legitimately reset the queue; a rejected send must never append.
    const reset = (await after.query.system.events()).some(({ event }) => event.section === 'gearEthBridge' && event.method === 'QueueReset');
    assert.equal((await after.query.gearEthBridge.queue()).toHex(), reset ? api.registry.createType('Vec<H256>', []).toHex() : (await before.query.gearEthBridge.queue()).toHex());
    assert.equal((await after.query.gearEthBridge.queueMerkleRoot()).toHex(), reset ? api.registry.createType('Option<H256>', '0x' + '00'.repeat(32)).toHex() : (await before.query.gearEthBridge.queueMerkleRoot()).toHex());
  }
  return result.status.asFinalized;
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
    assert(account.controller, 'Unknown local effective bonded signer');
    if (!afterRestart && account.controller === account.stash) {
      await submit(api.tx.balances.transferKeepAlive(account.controller.address, 1_000_000_000_000_000n), sudoSigner);
    }
  }
  const source = api.genesisHash.toHex();
  const chain = '0x' + '0'.repeat(62) + '01';
  const queue = '0x' + '03'.repeat(20);
  const zero = '0x' + '00'.repeat(32);
  const expected = u8aToHex(keccakAsU8a(u8aConcat(new TextEncoder().encode('vara/gear-eth-bridge-domain/v2'), api.genesisHash.toU8a(), hexToU8a(chain), hexToU8a(queue)), 256));
  const evidence = { phase: afterRestart ? 'after-restart' : 'before-restart', sourceGenesis: source, destinationChain: chain, destinationQueue: queue, bridgeDomain: expected,
    effectiveAccounts: accounts.map(({ name, nodeIndex, stash, controller }) => ({ name, nodeIndex, stash: u8aToHex(stash.publicKey), owner: u8aToHex(controller.publicKey) })), sessionTransitions: sessions, signedCommitments: proofs, mmrProofs };
  proofSubscription = await providers[0].subscribe('beefy_justifications', 'beefy_subscribeJustifications', [], (error, value) => {
    proofWork = proofWork.then(() => { if (error) throw error; return verifyProof(value); });
    proofWork.catch(() => {});
  });
  if (!afterRestart) {
    assert((await api.query.beefy.genesisBlock()).isNone, 'First phase requires inactive fresh BEEFY');
    assert((await api.query.gearEthBridge.destinationBinding()).isNone);
    assert.equal((await api.query.gearEthBridge.bridgeDomain()).toHex(), zero);
    const first = await register('initial-native-registration');
    const activationResult = await submit(api.tx.sudo.sudo(api.tx.beefy.setNewGenesis(8)), sudoSigner);
    const activation = (await api.rpc.chain.getHeader(activationResult.status.asFinalized)).number.toNumber() + 8;
    evidence.beefyGenesis = (await api.query.beefy.genesisBlock()).unwrap().toNumber();
    assert.equal(evidence.beefyGenesis, activation);
    const unbound = await waitCommitment('two-of-two native commitment before destination binding', beefyKeys(first), activation - 1);
    assert.equal(unbound.signatures, 2);
    assert((await api.query.gearEthBridge.destinationBinding()).isNone);
    assert.equal((await api.query.gearEthBridge.bridgeDomain()).toHex(), zero);
    await checkMmr(unbound, zero);
    evidence.sourceActivationBeforeBinding = true;
    await waitFinalized('legacy bridge initialized', async at => (await at.query.gearEthBridge.initialized()).isTrue);
    await submit(api.tx.sudo.sudo(api.tx.gearEthBridge.unpause()), sudoSigner);
    await submit(api.tx.sudo.sudo(api.tx.gearEthBridge.setFee(1_000_000_000_000n)), sudoSigner);
    assert((await api.query.gearEthBridge.transportFee()).toBigInt() > 0n);
    await bridgeSend('0x01');
    evidence.legacyLaneSend = true;
    await submit(api.tx.sudo.sudo(api.tx.gearEthBridge.pause()), sudoSigner);
    assert((await api.query.gearEthBridge.paused()).isTrue);
    const bound = await submit(api.tx.sudo.sudo(api.tx.gearEthBridge.bindDestination(chain, queue)), sudoSigner);
    const bindingBlock = (await api.rpc.chain.getHeader(bound.status.asFinalized)).number.toNumber();
    assert((await api.query.gearEthBridge.paused()).isTrue);
    assert.equal((await api.query.gearEthBridge.bridgeDomain()).toHex(), expected);
    assert.deepEqual((await api.query.gearEthBridge.destinationBinding()).unwrap().map(value => value.toHex()), [source, chain, queue]);
    await bridgeSend('0x02', 'gearEthBridge.BridgeIsPaused');
    evidence.pausedSendRejectedWithoutMutation = true;
    const signedBound = await waitCommitment('signed post-binding MMR snapshot', beefyKeys(first), bindingBlock);
    assert.equal(signedBound.signatures, 2);
    await checkMmr(signedBound, expected);
    await submit(api.tx.sudo.sudo(api.tx.gearEthBridge.unpause()), sudoSigner);
    assert((await api.query.gearEthBridge.paused()).isFalse);
    await bridgeSend('0x03');
    evidence.boundLaneSend = true;
  } else {
    const baselineHash = await api.rpc.chain.getFinalizedHead();
    const baselineAt = await api.at(baselineHash);
    const old = await queuedBundles(baselineAt);
    assert(await currentBundlesMatch(baselineAt, old), 'Restart changed active native bundle');
    assert(same((await baselineAt.query.beefy.nextAuthorities()).map(key => key.toHex()), beefyKeys(old)));
    for (const [index, account] of accounts.entries()) assert.equal(await providers[account.nodeIndex].send('author_hasSessionKeys', [old[index].toHex()]), true);
    evidence.beefyGenesis = (await baselineAt.query.beefy.genesisBlock()).unwrap().toNumber();
    assert.equal((await baselineAt.query.gearEthBridge.bridgeDomain()).toHex(), expected);
    assert.deepEqual((await baselineAt.query.gearEthBridge.destinationBinding()).unwrap().map(value => value.toHex()), [source, chain, queue]);
    assert((await baselineAt.query.gearEthBridge.paused()).isFalse);
    const bestBlock = (await api.rpc.chain.getHeader()).number.toNumber();
    const finalizedBlock = (await api.rpc.chain.getHeader(baselineHash)).number.toNumber();
    const beefyHead = await providers[0].send('beefy_getFinalizedHead', []);
    const commitmentBlock = (await api.rpc.chain.getHeader(beefyHead)).number.toNumber();
    await proofWork;
    const observedCommitment = Math.max(commitmentBlock, ...proofs.map(proof => proof.block));
    const setId = (await baselineAt.query.beefy.validatorSetId()).toString();
    evidence.restartBaseline = { bestBlock, finalizedBlock, commitmentBlock: observedCommitment, setId, currentKeys: old.map(keys => keys.toHex()), queuedKeys: old.map(keys => keys.toHex()) };
    const fresh = await waitCommitment('genuinely new two-of-two commitment after protected restart, before rotation', beefyKeys(old), Math.max(bestBlock, finalizedBlock, observedCommitment));
    assert.equal(fresh.signatures, 2);
    assert(BigInt(fresh.setId) >= BigInt(setId), 'Restart commitment regressed its set ID');
    evidence.freshRestartCommitment = fresh;
    await checkMmr(fresh, expected);
    await checkOwnership(baselineHash, old);
    // No owner-aware rotation occurs in this invocation until fresh restart signing is proven.
    const rotated = await register('post-restart-all-five-rotation');
    for (let index = 0; index < old.length; index++) for (const field of fields) assert(!old[index][field].eq(rotated[index][field]), 'Native rotation did not replace ' + field);
    const rotatedProof = await waitCommitment('native quorum after all-five rotation', beefyKeys(rotated), fresh.block);
    assert(BigInt(rotatedProof.setId) > BigInt(fresh.setId), 'Rotation did not advance BEEFY set ID');
    await checkMmr(rotatedProof, expected);
    await checkMmr(fresh, expected);
    await checkOwnership(baselineHash, old);
    const rotatedHash = await api.rpc.chain.getFinalizedHead();
    await checkOwnership(rotatedHash, rotated);
    const rotatedAt = await api.at(rotatedHash);
    for (const keys of old) {
      assert((await ownershipProof(rotatedHash, 'Babe', await rotatedAt.query.babe.currentSlot(), keys.babe)).isNone, 'Old BABE key remains current');
      assert((await ownershipProof(rotatedHash, 'Grandpa', await rotatedAt.query.grandpa.currentSetId(), keys.grandpa)).isNone, 'Old GRANDPA key remains current');
      assert((await ownershipProof(rotatedHash, 'Beefy', await rotatedAt.query.beefy.validatorSetId(), keys.beefy)).isNone, 'Old BEEFY key remains current');
    }
    evidence.oldKeysRejectedByCurrentOwnership = ['BABE', 'GRANDPA', 'BEEFY'];
    evidence.rotatedAllFiveKeys = true;
    evidence.ownershipApiProofs = ['BABE', 'GRANDPA', 'BEEFY'];
    evidence.wrongSetIdsRejected = ['GRANDPA', 'BEEFY'];
    const purged = accounts.at(-1);
    const purgedKeys = rotated.at(-1);
    const purgeSession = (await api.query.session.currentIndex()).toNumber();
    await submit(api.tx.session.purgeKeys(), purged.controller);
    const remaining = beefyKeys(rotated).slice(0, -1);
    await waitFinalized('purged validator absent from all five current and queued fields', async at => {
      const validators = (await at.query.session.validators()).map(owner => owner.toHex());
      const queued = await at.query.session.queuedKeys();
      const babe = await at.call.babeApi.currentEpoch();
      const grandpa = await at.call.grandpaApi.grandpaAuthorities();
      return !validators.includes(u8aToHex(purged.stash.publicKey))
        && !queued.some(([owner, keys]) => owner.eq(purged.stash.publicKey) || keys.toHex() === purgedKeys.toHex())
        && same((await at.query.beefy.authorities()).map(key => key.toHex()), remaining)
        && same((await at.query.beefy.nextAuthorities()).map(key => key.toHex()), remaining)
        && !grandpa.some(([key]) => key.eq(purgedKeys.grandpa))
        && !babe.authorities.some(([key]) => key.eq(purgedKeys.babe))
        && !(await at.query.imOnline.keys()).some(key => key.eq(purgedKeys.imOnline))
        && !(await at.query.authorityDiscovery.keys()).some(key => key.eq(purgedKeys.authorityDiscovery))
        && !(await at.query.authorityDiscovery.nextKeys()).some(key => key.eq(purgedKeys.authorityDiscovery));
    });
    const purgeProof = await waitCommitment('native quorum after validator purge', remaining, rotatedProof.block);
    assert(BigInt(purgeProof.setId) > BigInt(rotatedProof.setId));
    await checkMmr(purgeProof, expected);
    const afterPurgeHash = await api.rpc.chain.getFinalizedHead();
    const afterPurge = await api.at(afterPurgeHash);
    assert((await afterPurge.query.session.nextKeys(purged.stash.publicKey)).isNone, 'Purged registration remains');
    assert((await ownershipProof(afterPurgeHash, 'Babe', await afterPurge.query.babe.currentSlot(), purgedKeys.babe)).isNone, 'Purged BABE owner proof remains');
    assert((await ownershipProof(afterPurgeHash, 'Grandpa', await afterPurge.query.grandpa.currentSetId(), purgedKeys.grandpa)).isNone, 'Purged GRANDPA owner proof remains');
    assert((await ownershipProof(afterPurgeHash, 'Beefy', await afterPurge.query.beefy.validatorSetId(), purgedKeys.beefy)).isNone, 'Purged BEEFY owner proof remains');
    await checkOwnership(rotatedHash, rotated);
    assert.equal((await afterPurge.query.gearEthBridge.bridgeDomain()).toHex(), expected);
    sessions.push({ phase: 'purge', purged: u8aToHex(purged.stash.publicKey), submittedAtSession: purgeSession, currentAndQueuedAtSession: (await afterPurge.query.session.currentIndex()).toNumber() });
    evidence.purgedOwnerProofsRejected = ['BABE', 'GRANDPA', 'BEEFY'];
  }
  await proofWork;
  console.log(JSON.stringify(evidence, null, 2));
} finally {
  if (proofSubscription !== undefined) await providers[0].unsubscribe('beefy_justifications', 'beefy_unsubscribeJustifications', proofSubscription);
  await Promise.all(apis.map(node => node.disconnect()));
}
