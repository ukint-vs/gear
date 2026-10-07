# BEEFY deployment and real-network migration

## Existing testnet and mainnet: operator runbook

This section is the current source-chain procedure for the **11000 → 20100** migration, retaining each network's runtime identity (`vara` mainnet, `vara-testnet` testnet). The dated local/Hoodi sections below are historical evidence, not public-network deployment commands. **Installing a node release, enacting a runtime upgrade, activating BEEFY, and switching Ethereum bridge traffic are four separate actions.** Publishing the release does not perform the other three.

Run this procedure on the existing public testnet first, then repeat it independently on mainnet. Use operational testnet keys and valueless assets, not Alice/Bob keys. Do not copy testnet keys, genesis hashes, destination bindings or bootstrap checkpoints to mainnet. If either network is not on the supported predecessor, stop and qualify its actual state: do not force the 11000 migration or edit storage to resemble it.

### Release ownership and remaining gates

| Owner | Before proceeding | Evidence / stop condition |
| --- | --- | --- |
| Release team | Publish the reviewed node and network-specific normal-timing runtimes through the normal release pipeline; publish the offline proof helper and reviewed dependency lock | Exact commits, checksums, features, spec/transaction versions and predecessor. Mainnet runtime excludes dev; testnet runtime uses dev for its identity, not fast-runtime. Neither deployable WASM includes try-runtime or runtime-benchmarks. A separate production build during this review is not required. |
| Runtime maintainers | Qualify migration and added execution costs on supported hardware | Fresh full-state upgrade checks, preserved ownership proofs, measured weight/capacity evidence described below. The existing mainnet checkpoint is evidence, not a permanent release snapshot. |
| Network coordinator | Collect readiness from active, queued **and standby** registered owners; arrange independently operated proof archives | Per-stash registration or retirement record, finalized transaction hashes, node versions and keystore checks. Unresponsive legacy owners block activation until legitimately resolved. |
| Validators | Install the node before the upgrade; register the new key after it | Preserve the original four keys; keep the BEEFY secret local; wait for active and queued propagation. Do not send keys/seeds to the coordinator. |
| Governance | Authorize the runtime upgrade, immutable destination binding and later BEEFY activation separately | Approved network-specific calls, execution receipts and successful inner dispatch; an included governance wrapper alone is not success. |
| Bridge operators / reviewers | Approve destination contracts, replay/custody handover and durable relaying before real-fund enablement | A source-chain pass does not approve the Ethereum queue/token migration. The old message-only Hoodi deployment is not this approval. |

The production profile is a **release-pipeline responsibility**, not a demand that each validator builds Rust. The reviewed source logic carries into that build when revision and intended features are pinned; byte identity with a local release/try-runtime build is not assumed. Archive the checksums of the artifacts actually published and proposed on-chain. Use a same-revision, normal-timing **try-runtime companion** for migration APIs; do not upload that companion, and do not expect the production WASM to export try-runtime APIs. Replay/import the exact deployable WASM on staging as part of release acceptance.

The existing [release workflow](.github/workflows/release.yml) publishes separate artifacts. It currently copies the Rust version token verbatim into the filename: `2_01_00` corresponds to on-chain version **20100**. Use the exact filenames/checksums published by the chosen release, not an assumed numeric filename.

| Network | Node chain selector | Runtime identity / feature | Deployable runtime artifact |
| --- | --- | --- | --- |
| Mainnet | `vara` | `vara`, no `dev` | `production_vara_runtime_v2_01_00.wasm` |
| Public testnet | `vara-testnet` | `vara-testnet`, `dev`, **no fast-runtime** | `testnet_vara_runtime_v2_01_00.wasm` |

Both use production profile and normal two-hour sessions. The normal production node binary intentionally embeds the dev/testnet runtime; it still executes the actual on-chain mainnet WASM when running mainnet. Do not ban this expected node packaging or use its generated testnet WASM as the mainnet upgrade. Migration requires predecessor `spec_name` to match the chosen runtime; publish matching metadata and a matching companion for each network.

### Hardware and the specific weight qualification

The [official validator requirements](https://wiki.vara.network/docs/vara-network/staking/validate#hardware-requirements), checked while preparing this guide, are **2 vCPUs around 3.4 GHz (Intel Ice Lake or equivalent), 8 GB RAM, Ubuntu 22.04+ / GLIBC 2.35+, and at least 80 GB SSD with growth headroom**. They do not size an indexed full-history archive. No new validator hardware minimum is introduced by this guide; validators already meeting the baseline do not need a special BEEFY machine merely because benchmarks are outstanding.

Reference-hardware calibration means checking the runtime's execution accounting, not installing a different build profile or changing every validator's clock speed. Ref-time is charged in picoseconds relative to a reference machine; proof size is a separate byte dimension. This runtime budgets **1 second of ref-time per block** with **3-second slots**. The final full-state check declares **66.59%** migration weight; this proves the accounting fits, not actual WASM CPU time.

Maintainers should use a reproducible dedicated host and establish that the results are safe for the published validator baseline. Record CPU/model/frequency, virtualization, RAM, storage, OS, compiler/executor, commit, input sizes and repetitions; GHz alone does not establish equivalent performance. The current bridge weight header records a **Xeon Gold 6526Y**. The benchmark workflow requests a dedicated runner and a 3 GHz cap; its old runner is unavailable. The script's link to a 2020 upstream i7 machine is historical, not a new Vara hardware requirement. Provision an equivalent qualified runner or use a documented replacement; do not run CPU-tuning commands on live validators.

| Changed path | What must be measured and covered |
| --- | --- |
| One-off session-key migration | All legacy registered owners, current/queued snapshots, existing key-owner entries and exposure-sized historical tries on each target network; stress registry growth beyond the snapshot. Half-block trie headroom is **plus** size-dependent database charges, not a fixed total or owner cap. The 256 bridge limit does **not** cap migration inputs. |
| Every session / era transition | Immutable active-key snapshot, historical-root construction, exposure reads, retention/pruning and authority/MMR handover. Session rotation already charges the full maximum block weight; measure the whole transition block and other hooks rather than adding a second full-block charge. |
| Registration and purge | ECDSA proof verification, invalid-proof rejection, first/repeated registration and purge bookkeeping. Current registration allowance is 0.5 ms plus 4 DB reads / 3 writes and 128 KiB proof size, on top of base call weight. |
| Privileged binding / activation | Domain binding, current/queued validation and Merkle commitments up to 256 authorities. Activation reserves half the Operational maximum extrinsic weight; include the real governance wrapper's weight/admission. |
| Per-block MMR / bridge enqueue | MMR insertion, authority leaf and v2 bridge snapshot work; the extra cleanup-guard read/proof in message enqueue. Include queue/session-boundary cases, not only empty-block throughput. |

Acceptance is measured execution plus conservative margin within the charged allowances and full-block capacity on supported hardware, with storage/proof accounting reviewed. If an allowance is insufficient, update it and requalify before enactment. Do not automatically raise hardware requirements or suppress weight warnings. Tests/try-state checks and snapshot download time are not production hook timings.

The benchmark registry includes **`beefy_benchmarks`**: first registration, unproved/proved key rotation, failed-signature rejection, proved/legacy purge, activation, historical roots, session start and legacy migration. Bridge benchmarks include binding, last-free-slot enqueue and full-queue finalization. FRAME excludes fixture construction/signing and assertions from the timed block; storage-root recalculation is reported separately. The existing CI wildcard discovers these ordinary cases with verification enabled.

Calibration commands on the qualified host (production profile, the existing 50-step / 20-repeat cadence):

~~~sh
rtk cargo build -p gear-cli --profile production --locked \
  --features runtime-benchmarks,runtime-benchmarks-checkers
rtk target/production/gear benchmark pallet --chain=dev --steps=50 --repeat=20 \
  --heap-pages=16384 --pallet=beefy_benchmarks --extrinsic='*'
rtk target/production/gear benchmark pallet --chain=dev --steps=50 --repeat=20 \
  --heap-pages=16384 --pallet=pallet_gear_eth_bridge --extrinsic='*'
~~~

These are nondeployable benchmark artifacts: `dev` is the benchmark genesis, not fast timing. Activation samples 1..256 authorities; historical/session work samples 1..256 validators and 0..1024 nominators each through paged exposures. Migration samples 1..10,000 **total registered owners**, with separately bounded active/queued fixtures. The ranges are measurement samples, not protocol limits: compare fresh network inventory and exposures, extending the benchmark fixture range when needed. Measure a whole transition block with real hooks as well; individual custom-path benchmarks do not replace that capacity check. Retain upstream base weights and the explicit custom allowances until qualified measurements justify a change; session rotation already charges the full block allowance. See the [benchmark workflow](.github/workflows/benchmarks.yml) and [benchmark adapter](vara/runtime/vara/src/beefy_benchmarks.rs).

A local release-profile WASM stress run at 10,000 legacy owners exceeded the old fixed half-block reservation. The migration now adds upstream database charges for all validation/translation reads and per-owner writes; the accounting regression failed before this fix and passes after it. An oversized fresh snapshot must fail the upgrade-weight gate, not be forced through or repaired with a standby admission cap. These samples are execution diagnostics on an M4 Max, not reference-hardware calibration.

### 1. Coordinator: publish a network-specific release manifest

Record the exact release/commit/artifact checksums; source chain spec and block-zero hash; finalized predecessor hash/version/state root; expected 20100 metadata; destination Ethereum genesis and chain ID; approved queue address; governance proposal/execution references; upgrade window and registration deadline; archive endpoints/operators; and the activation and bridge-cutover approval owners. Mainnet selects `--chain vara`; public testnet selects `--chain vara-testnet`. Use an explicit approved spec if the release names another test network. Never infer the network from the node's default.

Inventory all `Session.NextKeys` owners, `Session.Validators` and `Session.QueuedKeys` at a finalized block and retain the original four keys. The recorded mainnet sample had 112 registered owners and 59 active validators; these are **historical counts, not constants**. Refresh near enactment and requalify material changes. Resolve absent owners before committing to an activation date; there is no safe counter-reset shortcut.

Create a fresh full snapshot on **each** network and run the same-revision try-runtime companion with CLI 0.10.1:

~~~sh
: "${SOURCE_WS:?Set the network RPC}" "${PRE_UPGRADE_FINALIZED_HASH:?Pin finalized state}"
: "${SNAPSHOT:?Choose a new snapshot path}" "${TRY_RUNTIME_WASM:?Select the matching companion}"
rtk proxy try-runtime create-snapshot --uri "$SOURCE_WS" \
  --at "$PRE_UPGRADE_FINALIZED_HASH" "$SNAPSHOT"
rtk proxy try-runtime --runtime "$TRY_RUNTIME_WASM" on-runtime-upgrade \
  --blocktime 3000 --checks all --disable-mbm-checks snap -p "$SNAPSHOT"
~~~

First verify version **11000** and the network-specific `spec_name` (`vara` or `vara-testnet`) at the pinned hash, and compare the unmodified snapshot root to that header's state root. Keep the snapshot, logs, proof-retention evidence and hashes in durable release storage. Only multi-block migration simulation is disabled because this runtime has no multi-block migrator; retain spec, idempotence, decoding and weight checks. On testnet, rehearse the actual existing-state upgrade, real registration delay, session/era handovers and node/relay restart before scheduling mainnet. The four-slot local smoke is not a substitute and must never be pointed at a public network.

### 2. Validators and archive operators: before the runtime upgrade

1. Verify the published binary checksum/version, install it under the existing service account, and perform a rolling restart coordinated to preserve BABE/GRANDPA quorum. Keep the existing chain spec, database/base path, keystore path/password and network identity. Do not purge/resync as an upgrade step, regenerate the four legacy session keys, reset genesis, or run two active signing nodes with the same keys.
2. Preserve encrypted backups of the keystore and its password under existing custody procedures. Keep all still-active/queued old keys during the transition. The libp2p identity, account signing keys and five session keys have different purposes; BEEFY does not replace the stash/controller or node identity key.
3. Keep validator mode enabled and BEEFY networking allowed. Do not disable GRANDPA. Keep unsafe authoring RPC loopback-only behind the operator's existing access controls; never expose it publicly to insert keys.
4. Add `--enable-offchain-indexing true` **before the upgrade**. The coordinator must have at least two independently operated proof-serving nodes with `--state-pruning archive --blocks-pruning archive`, indexed from the first MMR insertion. Ordinary validators need not all become full archives. Archive flags do not restore previously pruned state or backfill missing offchain MMR nodes; a late proof server needs verified replay/recovery, not just a flag change.
5. Verify advancing finalized blocks and matching finalized hashes against another operator, and report the running node version/readiness. Installing the binary does not change the runtime stored on-chain. Before the runtime upgrade, `SessionKeys` is still four-key encoding; **do not submit a five-key bundle or the new proof yet**. A BEEFY secret may be prepared/imported early, but registration waits for the new runtime.

### 3. Governance and coordinator: enact and inspect the runtime upgrade

Enact the approved **network-specific release WASM** through that network's existing runtime-upgrade governance mechanism. Use the testnet artifact only on testnet and the production artifact only on mainnet; do not substitute local Sudo or raw storage edits. After finalization, verify on-chain code against the published artifact, version **20100 with unchanged network identity**, and expected metadata. Verify uninterrupted BABE production / GRANDPA finality, preserved four-key ownership and active/queued order, preserved bridge queue/nonces, and the initial unproved-owner count. The migration adds placeholders; `Beefy.GenesisBlock` must still be absent, and a first-upgrade lane must not already have a conflicting binding. Discover and record first MMR insertion block **A**; it is not later BEEFY activation block **G**.

A failed migration or unexpected predecessor/state is a release stop, not permission to bypass a guard. An on-chain runtime change is not undone by reinstalling the old node binary.

### 4. Each registered validator: prepare and register the real BEEFY key

These steps apply to standby owners too. Use the release's `beefy-validator-tools.tar.gz` bundle, verify its published SHA-256, and extract it into a private working directory. It contains this runbook, the helpers and the reviewed `scripts/package-lock.json`. Use Node **22.19.0** / npm **10.9.3** and run `rtk proxy npm ci --prefix scripts --ignore-scripts --no-audit --no-fund`, then `rtk proxy npm test --prefix scripts`, before handling secrets. Run commands from the extracted bundle or the checked-out release repository. `PROOF_DEPS` is the absolute path to its `scripts` directory. Do not regenerate the lock or upgrade dependencies during rollout.

**Read-only input/readiness report.** Set `SOURCE_WS` to your node, `EXPECTED_GENESIS` and `EXPECTED_SPEC_NAME` (`vara` or `vara-testnet`) from the release manifest, and `STASH` to your public stash address. Run from the checked-out release repository. This report uses one finalized state and contains no private keys. It reads the three custom storage aliases through raw RPC because they are not pallet metadata storage entries; a missing counter is an error, not zero. The current public-testnet predecessor has not been independently qualified by this runbook: operators must supply a reachable RPC and verify its exact identity/version before using the testnet artifact.

~~~sh
rtk proxy env PROOF_DEPS="$PROOF_DEPS" SOURCE_WS="$SOURCE_WS" \
  EXPECTED_GENESIS="$EXPECTED_GENESIS" EXPECTED_SPEC_NAME="$EXPECTED_SPEC_NAME" \
  STASH="$STASH" node --input-type=module <<'NODE'
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(process.env.PROOF_DEPS + '/package.json');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { xxhashAsU8a, blake2AsU8a, decodeAddress } = require('@polkadot/util-crypto');
const { u8aConcat, u8aToHex, hexToU8a } = require('@polkadot/util');
for (const name of ['SOURCE_WS', 'EXPECTED_GENESIS', 'EXPECTED_SPEC_NAME', 'STASH']) assert(process.env[name], name);
const provider = new WsProvider(process.env.SOURCE_WS);
const api = await ApiPromise.create({ provider, noInitWarn: true });
try {
  assert.equal(api.genesisHash.toHex(), process.env.EXPECTED_GENESIS.toLowerCase());
  const head = await api.rpc.chain.getFinalizedHead();
  const at = await api.at(head);
  const version = await api.rpc.state.getRuntimeVersion(head);
  assert(['vara', 'vara-testnet'].includes(process.env.EXPECTED_SPEC_NAME));
  assert.equal(version.specName.toString(), process.env.EXPECTED_SPEC_NAME);
  assert.equal(version.specVersion.toNumber(), 20100, 'Use the approved release metadata');
  const stash = decodeAddress(process.env.STASH);
  const bonded = await at.query.staking.bonded(stash);
  const registered = await at.query.session.nextKeys(stash);
  assert(bonded.isSome && registered.isSome, 'Retired/unregistered owner: use the coordinator procedure');
  const keys = registered.unwrap();
  assert.equal(keys.toU8a().length, 161, 'Must be the post-upgrade five-key bundle');
  const storageKey = (pallet, name, ...suffix) => u8aToHex(u8aConcat(
    xxhashAsU8a(pallet, 128), xxhashAsU8a(name, 128), ...suffix));
  const raw = key => provider.send('state_getStorage', [key, head.toHex()]);
  const pending = await raw(storageKey('Session', 'PendingRegistrations'));
  const active = await raw(storageKey('Historical', 'ActiveSessionKeys'));
  assert(pending !== null && active !== null, 'Missing migration bookkeeping; stop');
  assert.equal(pending.length, 10, 'PendingRegistrations must be a SCALE u32');
  const proven = await raw(storageKey('Session', 'RegisteredKeys', blake2AsU8a(stash, 128), stash));
  const activeKeys = api.registry.createType('Vec<(AccountId, ' + keys.toRawType() + ')>', hexToU8a(active));
  console.log(JSON.stringify({
    sourceGenesis: api.genesisHash.toHex(), finalizedHash: head.toHex(),
    specName: version.specName.toString(), specVersion: version.specVersion.toNumber(), stash: process.env.STASH,
    signer: bonded.unwrap().toString(), sessionKeys: keys.toHex(), beefyPublic: keys.beefy.toHex(),
    pendingRegistrations: api.registry.createType('u32', hexToU8a(pending)).toNumber(),
    provenBeefyPublic: proven, currentSession: (await at.query.session.currentIndex()).toNumber(),
    activeSessionKeys: activeKeys.toJSON(), queuedKeys: (await at.query.session.queuedKeys()).toJSON(),
    currentAuthorities: (await at.query.beefy.authorities()).toJSON(),
    nextAuthorities: (await at.query.beefy.nextAuthorities()).toJSON(),
    beefyGenesis: (await at.query.beefy.genesisBlock()).toJSON(),
    destinationBinding: (await at.query.gearEthBridge.destinationBinding()).toJSON()
  }, null, 2));
} finally {
  await api.disconnect();
}
NODE
~~~

Use the returned `signer` (from `Staking.Bonded`), not an assumed stash/controller address. Fund that account with enough **liquid unbonded** balance for fees. Preserve the returned 161-byte `sessionKeys` bundle: first 128 bytes are the four original keys in runtime order, last 33 bytes are BEEFY. If registration is absent or bonding has changed, stop and reconcile the owner; do not construct a bundle from memory.

Prepare a unique secp256k1 secret using your approved secure key-generation procedure. The protected file must contain exactly `0x` plus 64 hex characters (optional final newline), mode 0600, in a private directory. Do not reuse the testnet secret on mainnet or use a stash/controller secret. Keep it out of argv, environment, terminal output, tickets and the coordinator's records. The file/backup must remain available for future ownership proofs; `author_rotateKeys` alone cannot sign this custom registration proof from a keystore-only BEEFY key.

With `SEED_FILE`, `NODE_BINARY`, `CHAIN` and `BASE_PATH` set to your protected file and actual service configuration, import it locally under the same OS user and keystore options used by the node, during a coordinated single-node maintenance window:

~~~sh
rtk "$NODE_BINARY" key insert --chain "$CHAIN" --base-path "$BASE_PATH" \
  --key-type beef --scheme ecdsa --suri "$SEED_FILE"
rtk proxy node scripts/beefy-session-proof.mjs \
  --genesis "$EXPECTED_GENESIS" --controller "$SIGNER" \
  --session-keys "$SESSION_KEYS" < "$SEED_FILE" > "$PUBLIC_PROOF_JSON"
~~~

Here `SIGNER` and `SESSION_KEYS` are copied from the finalized report, not guessed. If the service uses a custom `--keystore-path` or `--password-filename`, pass the same options to key insertion (never put a password in argv). Restart the node and verify local `author_hasKey(beefyPublic, "beef")` and `author_hasSessionKeys(sessionKeys)` against the helper's output. A true result proves presence, not successful native signing. Avoid typed `Text` conversion when importing hex seeds by RPC; CLI file import avoids the known SURI conversion pitfall.

Using the post-upgrade metadata in a trusted signing UI, submit **session.setKeys(keys = helper.sessionKeys, proof = helper.proof)** from the actual bonded signer. The proof is 65 bytes, not `0x`. Use the generic extrinsic interface if a staking wizard still submits an empty proof. For proxy/multisig use, the proof must bind the effective signed origin dispatched to Session, not the outer fee payer. Verify finalization **and dispatch success**, including any inner wrapper result; retain the extrinsic hash and public bundle. Re-run the report: `Session.NextKeys` must exactly match the submitted bundle and `provenBeefyPublic` must equal its BEEFY key. Never send the ECDSA secret or account seed to anyone collecting readiness.

If the signing UI presents five separate key fields, retain the four original `Session.NextKeys` fields and use the helper's `beefyPublic` for BEEFY. Confirm their SCALE encoding equals the helper's complete `sessionKeys` before signing; do not invoke another key rotation between producing the proof and submitting the bundle.

**Retired owners:** the owner may call `session.purgeKeys()` instead of registering. If `Staking.Bonded` is absent, the runtime supports the stash itself as the purge signer. First coordinate retirement and verify the validator is no longer needed in active/queued sets; do not mass-purge active validators to make the counter zero. Purging is not unbonding and does not immediately remove queued keys. An unavailable owner requires a separately reviewed governance/remediation plan; never decrement `PendingRegistrations` by hand.

### 5. Coordinator: wait for activation readiness

Record public-only status per stash: operator contact, node release, original four keys, new BEEFY public key, proof/registration transaction, finalized inclusion/session, active and queued observations, and retirement where applicable. Re-evaluate at a common finalized hash; do not mix current and queued data from different heads.

- `Session.PendingRegistrations` must equal **0**, including all legacy standby owners. In the report, a nonzero counter is a blocker; absence is also a blocker. New valid standby registrations are permissionless and do not increment it.
- Confirm actual ordered `Historical.ActiveSessionKeys` and `Session.QueuedKeys` match `Beefy.Authorities` and `Beefy.NextAuthorities` respectively. Every member must have the matching `Session.RegisteredKeys` proof record, a valid non-placeholder unique key and a working signer. Standby owners need proven registration, not election into the active set.
- Require 1..256 actual current and queued authorities, `Staking.ValidatorCount <= 256`, correct `Beefy.SetIdSession` and exact `MmrLeaf.BeefyAuthorities` / `BeefyNextAuthorities` IDs, lengths and roots. Governance must maintain the 256 bridge policy at future elections; the source's 100,000 storage bound is not the Ethereum verifier limit.
- Observe successive normal-timing session transitions and an era transition without loss of finality. The normal session is **2 hours**, the era **6 sessions / 12 hours**. Newly registered keys are for the session after next, subject to selection and inclusion timing; allow multiple hours and rely on finalized active/queued state, not a fixed wall-clock promise or an RPC `hasKey` result.
- Verify proof archives can actually serve historical MMR data starting at **A**. Persist the authority handover data and queue proofs needed by the relayer before retention/rollover loses availability.

### 6. Governance: bind the destination, then schedule BEEFY

Once the bridge team has approved the intended lane and real-fund cutover/replay strategy, Root governance calls **gearEthBridge.bindDestination(chainId, queue)** exactly once. `chainId` is the destination Ethereum numeric chain ID encoded as a **32-byte big-endian H256**, not a source-chain ID, little-endian integer or genesis hash. `queue` is the approved 20-byte queue address. If deployment depends on a later BEEFY bootstrap, the address must come from a reviewed deterministic deployment plan; do not guess it from transaction nonces. Binding cannot be changed later to repair a mistaken address.

After finalization, verify `DestinationBound`, `GearEthBridge.DestinationBinding` and `BridgeDomain` against `Keccak256("vara/gear-eth-bridge-domain/v2" || sourceGenesis[32] || chainIdBE[32] || queue[20])`. This binds identity; it does not prove that the destination bytecode is deployed, compatible or authorized to move funds. The historical v1 Hoodi client must not be assumed compatible with the current **v2 domain-bound bridge snapshot**.

Only after the full readiness check succeeds, schedule **beefy.setNewGenesis(delayInBlocks)** through the approved Root-governance route. Use a positive delay long enough for operational observation; **G is execution block + delay**, not proposal block + delay. The call is Operational and still needs room for its real governance wrapper. Its internal checks reject unready registration, authority commitments or bindings even when dispatched through Root/bypass filtering. Verify successful dispatch and finalized `Beefy.GenesisBlock`; do not repeatedly resubmit when a vote or transaction is merely pending.

After **G**, require advancing native BEEFY finality, cryptographically valid quorum commitments and a real authority-set handover, compared across independent nodes. Authenticate bootstrap **C > max(A, G)**, the newest MMR leaf, its actual parent timestamp and ordered authority tuples. Monitor BABE/GRANDPA alongside BEEFY. A stored activation block or a running process is not proof that validators can sign.

### 7. Bridge operators: enable traffic separately, and retain recovery

The bridge team must separately deliver reviewed v2-compatible contracts/relay, authenticated bootstrap and subsequent accepted roots, durable handover catch-up and restart recovery, governance authorization, queue/asset cutoff and **preserved consumed-nonce/replay state**. Preserve or explicitly migrate custody and pending messages; an empty new queue can replay an already-consumed legacy message because message hashing did not change. Do not run old/new queues as unrestricted authorities for the same assets. Start with shadow observation and a valueless/capped canary, then approve token traffic after real maturity/finality and replay checks in both directions. Keep Ethereum-to-Gear infrastructure running.

On missing owners, failed signatures, archive gaps, source disagreement, wrong binding or missing destination approvals: stop the next step and preserve state/evidence. Before activation, keep the upgraded runtime running with BEEFY inactive. After activation, never treat a repeated `setNewGenesis`, reverting the node binary, wiping the MMR, or resetting a queue as rollback; recovery requires a coordinated plan compatible with the destination client's trust and replay ledger. Keep old session keys until their active/queued duty ends and retain secure backups according to the offence-proof/recovery policy; on-chain `purgeKeys` is not keystore deletion.

### Current evidence versus remaining operational work

The source implementation has passed 126 runtime/pallet tests, 56 native-client tests, full-state migration checks at the recorded mainnet predecessor, 119 retained real-state ownership proofs and a two-node activation/rotation/purge rehearsal with eight verified commitments. Those are completed checks. Remaining work is owned in the table above: release publication, measured weight/capacity qualification, **actual** network-wide key registration/propagation, governance execution and normal-timing public-testnet acceptance, plus the independently reviewed Ethereum migration/relay/custody/replay cutover. No validator needs to rebuild the runtime or benchmark the entire chain individually.

Runbook verification: the read-only report was executed on an isolated native node and checked against its actual single unproved owner, little-endian counter value **1**, and matching active/queued keys; all **25** shell examples across the deployment/migration guides and node README passed `bash -n`. The fixture was stopped afterward. Public mainnet was independently observed at `vara/11000`; the historical `testnet.vara.network` and `testnet-archive.vara.network` endpoints did not resolve from this environment. This is not proof that testnet is unavailable everywhere: the coordinator must provide a working approved endpoint and qualify its own predecessor snapshot. No public-network transaction was submitted.

The pinned helper was subsequently exercised end-to-end by the native smoke: `npm ci --prefix scripts --ignore-scripts --no-audit --no-fund` and all three offline test groups passed; both validators registered proofs produced by the actual stdin-only helper. Activation occurred at block **35**; registration propagated from session **5 → 8**, rotation **10 → 13**, and purge **13 → 16**. Eight native quorum commitments verified, with all three ownership APIs and wrong-set/purged-owner rejection passing. Both disposable nodes were stopped. Evidence SHA-256: `cf204a95b579e45e97578def3e6c88684347e7d105ffbb36543da9404cc5dbd5` (`/tmp/gear-beefy-final-tools-evidence.json`).

## Historical local and Hoodi deployment evidence


## Current message-only deployment (2026-09-22)

The isolated local Gear → Hoodi message bridge is now implemented and deployed. The new command is `beefy-relay hoodi`; `rehearse` remains Anvil-only. The current operator commands and limits are in `/Users/ukintvs/Documents/projects/gear-bridges-beefy-e2e/README.md`, under **Local authorities to Hoodi**.

- Source RPCs: `ws://127.0.0.1:9944` and `ws://127.0.0.1:9945`. Persistent databases: `~/.local/share/vara-beefy-hoodi/{alice,bob}`. Do not reset them while reusing this deployment.
- Hoodi client: `0xa757bc9fbadf26c6f4b6d42a6f5c5c19396a2fce`. Adapter: `0xede57a9e316ed06833255c54c41e6447c158170f`.
- Queue: `0xab51a342680b774137a8231cbcebb50417ab9aeb`. Test receiver: `0xd1be52429eb7a50c5f763538664fe7b85ab01ffc`.
- Test wallet: `0x4E192AF047774c6E5172a59e99Ded6E5b49b1534`. Local wallet JSON is outside the repositories at `~/.config/vara-beefy-hoodi/test-wallet.json`, mode 0600. Never publish its contents or use it for real assets.
- Finalized acceptance evidence: `~/.local/share/vara-beefy-hoodi/evidence/verification.json`, status `passed`, finalized Hoodi block **3675439**. Both source messages (blocks 654 and 662) were delivered and replay-rejected; early delivery and stale-anchor rejection passed. Natural queue clearing advanced queue ID 0 → 1. Mutable recovery/follower state remains in `state.json` beside the snapshot.

Persistent supervised processes: `hoodi-gear-alice`, `hoodi-gear-bob`, and `hoodi-beefy-relay`. The follower was observed advancing through source block 735 / authority set 92 after verification. It maintains handovers, not arbitrary application-message delivery; it spends Hoodi gas and stops on errors. Keep the host awake, retain both databases, and stop the follower before rerunning the command against its locked evidence directory. The client freshness policy is 24 hours; this demo provides no expired-client recovery.

Additional checks passed: 13 Rust tests, 168 Solidity tests, and the complete two-node/Anvil regression rehearsal. Changes remain local and uncommitted.

The runner retains two messages, rotates Alice's BEEFY key twice, resumes after process restart, and exercises real maturity, replay rejection, stale-anchor rebuilding and natural queue clearing. `--follow` maintains authority handovers after verification. Ambiguous deployment or source submission interruptions fail closed and require reconciliation. This is not a production relay, token bridge, or integrated Gear governance deployment.

The remaining sections preserve the **2026-09-21 planning baseline**. Statements below about missing Hoodi tooling are superseded for this bounded message-only demonstration; production durability, migration, token and governance gates remain open. Read the [migration roadmap](beefy-migration.md) before planning a live-chain migration.

## Supported environments

| Environment | Current support | Appropriate use |
| --- | --- | --- |
| Two local Gear authorities + local Anvil, chain ID 31337 | Implemented and exercised by `beefy-relay rehearse` | Fresh-chain consensus/message interoperability |
| Persistent local Gear + Hoodi, chain ID 560048 | Implemented by `beefy-relay hoodi`; deployed message-only demonstration | Isolated, valueless consensus/message verification, not token bridging |
| Existing Vara testnet or mainnet + Ethereum | Source migration implemented and snapshot-qualified; network rollout and Ethereum cutover separately gated | Follow the current operator runbook above, not the historical local commands |

**Do not substitute a Hoodi URL into `rehearse`.** Use the separate `hoodi` command. `BeefyLocal.s.sol` remains restricted to chain 31337; `BeefyHoodi.s.sol` requires 560048. The generic `Deployment.s.sol` still uses Base's legacy `VerifierTestnet` on Hoodi, not the BEEFY adapter.

The local runner deploys mock token/application configuration for a message test. It does not establish a complete, operational token bridge with real governance and reverse-direction programs.

## 1. Select source revisions and prerequisites

At the 2026-09-21 inspection, secure-v1 exists as local uncommitted changes in `gear-beefy-e2e` and `gear-bridges-beefy-e2e`. The pushed branch heads listed in the migration roadmap are older baselines. Use those existing validated worktrees until the paired v1 changes are published. For a reproducible team deployment, first pin the reviewed commits that contain v1; do not assume a fresh clone of the current draft PR heads contains it.

Examples below use Bash and the workspace's `rtk` command wrapper. `rtk proxy` preserves raw command output for JSON pipelines. Install `rtk` for these literal commands, or invoke the underlying tools directly in a workspace without that wrapper. No protocol component depends on `rtk`.

Set paths to the actual worktrees:

~~~sh
export WORKSPACE="$HOME/Documents/projects"
export GEAR_DIR="$WORKSPACE/gear-beefy-e2e"
export BRIDGE_DIR="$WORKSPACE/gear-bridges-beefy-e2e"
rtk git -C "$GEAR_DIR" status --short --branch
rtk git -C "$BRIDGE_DIR" status --short --branch
~~~

Prerequisites:

- Gear's pinned `nightly-2026-07-21` Rust toolchain and Bridge's pinned `nightly-2025-10-20`, including their configured WASM targets. Run Cargo in the corresponding repository so its pin applies.
- Gear's native build prerequisites and `cargo-nextest`; use the Gear repository's setup instructions and `make init` as needed.
- Bridge's native build dependencies, including compiler/LLVM, CMake, OpenSSL and protobuf requirements described in [Running the bridge](running-the-bridge.md).
- Forge, Cast and Anvil 1.7.x; the verified local run used 1.7.1. Node/npm/npx are needed by the existing OpenZeppelin upgrade validation invoked from Foundry.
- The checked-in Soldeer dependency versions; do not replace them with arbitrary latest packages.
- `jq` for evidence inspection and `curl` for the optional persistent-node RPC examples.

On the macOS workspace used for the recorded verification, Cargo required a temporary `/tmp/gear-cc-mixed` compiler wrapper: LLVM 18 for compilation and Apple clang for linking. If that already-prepared executable exists, export `CC=/tmp/gear-cc-mixed` before Cargo commands. It is not a repository file or a portable prerequisite; on another machine follow the native toolchain setup rather than copying a nonexistent path or suppressing native builds.

## 2. Build and verify the local implementation

From Gear:

~~~sh
cd "$GEAR_DIR"
rtk cargo nextest run -p vara-runtime -E 'test(bridge_leaf::tests) | test(bridge_finalization_at_n_is_committed_at_n_plus_one) | test(session_boundary_leaf_uses_new_authorities_and_preclear_bridge_root)'
rtk cargo nextest run -p vara-runtime
rtk cargo nextest run -p vara-runtime --features dev
rtk cargo build -p gear-cli --release --features fast-runtime --target-dir target/beefy-e2e
~~~

`fast-runtime` is only for explicit dev/local chain IDs. The local slot is 3000 ms, with four-slot epochs and a one-block BEEFY minimum interval. Do not build a production node with this feature. It does not lower the Solidity freshness or sampling policy.

From Bridge, install contract dependencies if absent, build the artifacts and compile against the matching ABI:

~~~sh
cd "$BRIDGE_DIR/ethereum"
rtk forge soldeer install
cd "$BRIDGE_DIR"
rtk forge clean --root ethereum
rtk forge build --root ethereum --force
rtk cp ethereum/out/BeefyClient.sol/BeefyClient.json api/ethereum/BeefyClient.json
rtk cp ethereum/out/VaraQueueRootVerifier.sol/VaraQueueRootVerifier.json api/ethereum/VaraQueueRootVerifier.json
rtk cargo test -p beefy-relay
rtk forge test --root ethereum --match-path 'test/*Beefy*.t.sol' -vvv --gas-report
~~~

Before deployment-inclusive verification, clean and rebuild again. Switching between targeted and full Foundry compilation can leave duplicate `out/build-info` records that OpenZeppelin's upgrade validator rejects. Do not bypass upgrade validation to work around that error.

~~~sh
rtk forge clean --root ethereum
rtk forge build --root ethereum --force
rtk forge test --root ethereum
rtk cmp api/ethereum/BeefyClient.json ethereum/out/BeefyClient.sol/BeefyClient.json
rtk cmp api/ethereum/VaraQueueRootVerifier.json ethereum/out/VaraQueueRootVerifier.sol/VaraQueueRootVerifier.json
rtk cargo build -p beefy-relay --release
rtk target/release/beefy-relay rehearse --help
~~~

The verified v1 baseline passed 31 normal runtime tests, 34 `dev` runtime tests, 12 relay tests and 164 Foundry tests, including 36 BEEFY/client/queue tests. Later revisions may add tests; successful exits and the exercised behavior matter more than matching these historical counts.

Only deliberately regenerate shared fixtures after changing the protocol/generator:

~~~sh
UPDATE_BEEFY_FIXTURE=1 rtk cargo test -p beefy-relay --lib fixtures
~~~

Generated fixture timestamps and development keys are test data, not a deployable source checkpoint.

### Mainnet migration input and external release gates (2026-10-07)

The read-only acquisition described in [the migration checkpoint](beefy-migration.md#existing-mainnet-input-checkpoint-2026-10-07) pins finalized block 36,741,398 / vara spec 11000. The complete required-pallet snapshot is `/tmp/vara-migration-complete-11000-a83455.snap`; it is not a whole-chain state snapshot. The full-chain scraper targets `/tmp/vara-mainnet-11000-a83455.snap`. Keep snapshots and evidence private and archive them durably: `/tmp` is not a release archive.

Release requires all of the following, not merely a green local fixture suite:

1. Publish the reviewed normal-timing node and network-specific runtimes through the normal production-profile release pipeline and record hashes; validators install that release. Mainnet WASM excludes dev; testnet WASM uses dev with normal timing. Both exclude fast-runtime, runtime-benchmarks and try-runtime. The node's embedded testnet runtime is expected. Use the matching companion for migration APIs and the exact deployable WASM for staging.
2. Run exact-predecessor migrations, idempotence, storage decoding and weight checks against real finalized state. `.github/workflows/build.yml` now always pins a finalized RPC block, verifies `vara/11000` there, and creates a real full snapshot; it never substitutes the obsolete download URL or a fixture. Standard CLI spec-version checking permits `new >= existing`, so it alone does not prove the exact predecessor. No spec-check or weight-warning suppression is permitted in this gate.
3. Complete whole-chain `--checks all`, preserve old and new ownership proofs, and rehearse successive normal-timing session/era transitions and uninterrupted BABE/GRANDPA/BEEFY finality. Scoped `pre-and-post` success does not establish unrelated pallet invariants or live consensus liveness.
4. Complete the specific maintainer-owned weight/capacity qualification in the current operator runbook above. The existing validator baseline remains the published Ice Lake-class 2-vCPU / 8-GB requirement, not a new machine purchase. Conservative declared allowances and M4 diagnostics are not measured reference-hardware weights.
5. Reconcile every legacy registered key owner (112 in the historical mainnet sample, not a fixed release count), including standby owners. Require `PendingRegistrations == 0`, operational proven keys in active and queued sets, the approved destination domain, indexed archives and governance authorization. Keep future elected sets <=256 for the current Ethereum verifier without silently reducing the source consensus storage bound.
6. Complete the remaining queue/asset/replay, destination-contract, durable-relay and recovery gates in the migration roadmap. None of the read-only work authorizes a live upgrade, validator transaction, destination binding or asset movement.

Installed qualification CLI and CI now both use `try-runtime-core 0.10.1`; the Linux release asset was resolved from the actual upstream v0.10.1 release. Execute the workflow's full-chain gate with that pinned tool before release.

### Reproducing the final counter-aware read-only qualification

Immutable normal-timing artifacts are saved under `/tmp/vara-qualified-counter-20100-a83455/`: `vara_runtime.wasm` SHA-256 `4417279b59b766c1ad894db989b6d47141f33e58c2f8f83e3cb11e5e8467b5a0`, `libvara_runtime-3714ad8500c6c51b.rlib` SHA-256 `ab1cd33c027025d57cf313777970cbed2940126162fc7b2b0283ec46c624341a`, and `native-fingerprint.json` recording normal default/std/try-runtime features. The native helper `/tmp/vara-mainnet-inspect.rs` links the saved rlib and its matching SDK dependencies (`frame-support-e0f989aedd54f096`, `pallet-babe-1df9beb972835589`), not a fast/dev runtime. Its executable is `/tmp/vara-mainnet-counter-final`. Preserve these inputs with the snapshots before releasing; ephemeral paths are evidence locations, not committed tooling.

The helper reconstructs genuine snapshot externalities, asserts exact vara/11000 predecessor and local 20100, executes the full Executive migration, verifies the 112 pending legacy registrations and historical roots, invokes actual runtime ownership-proof APIs, and verifies an actual predecessor-issued BABE proof after migration/rotation. The public proof file was obtained via read-only `BabeApi_generate_key_ownership_proof` at the pinned block and contains no signing material. Final exercised commands (both exit 0):

~~~sh
rtk proxy env VARA_OLD_PROOFS=/tmp/vara-old-issued-proof-a83455.txt \
  /tmp/vara-mainnet-counter-final /tmp/vara-migration-complete-11000-a83455.snap \
  --upgrade --rotate > /tmp/vara-mainnet-native-final.log 2>&1
rtk proxy try-runtime \
  --runtime /tmp/vara-qualified-counter-20100-a83455/vara_runtime.wasm \
  on-runtime-upgrade --blocktime 3000 --checks pre-and-post --disable-mbm-checks \
  snap -p /tmp/vara-migration-complete-11000-a83455.snap \
  > /tmp/vara-mainnet-wasm-final.log 2>&1
~~~

Only the multi-block-migration check is disabled because this runtime has no multi-block migrator; spec, idempotence, decode and weight checks are enabled. The native log records 112 pending registrations, all 85 prior roots preserved, 119 actual-state proofs surviving rotation, BEEFY inactive and no destination binding. It records migration wall/CPU **132.938/132.588 ms**, rotation **2.167/2.170 ms**. WASM records **0.501825 s / 50.18%** aggregate declared ref-time usage within max **1 s**, compressed PoV **149.0 KiB**, and identical second-upgrade roots `0xcdc9d6150f5e98f72d1ad9dfc8796cccce28e5ec43a0c2fff23e8adc8a91cb37`. These do not establish calibrated reference-hardware performance or live finality.

Full-chain acquisition completed at the same pinned hash:

~~~sh
rtk proxy try-runtime create-snapshot --uri wss://rpc.vara.network \
  --at 0xa83455d6fcdf6c72f1cedad6117ae86dedd8e9716c1755c4f26bd0f13b9256d8 \
  /tmp/vara-mainnet-11000-a83455.snap
~~~

The complete snapshot has **798,801** storage keys and **2,084,893,951** bytes; SHA-256 `d7089929dc7dfa2dfb6c05395708c9c95a2c52c076cdab2b76d6fe4e18cbe25a`. Final normal-timing try-runtime WASM SHA-256 `329cf47b95a5aa6ffc7da62a078894aeee0f00e9169a20bf659b5dff90cbd018` passed the following whole-chain check with exit **0**, not just the scoped pre/post check:

~~~sh
rtk proxy try-runtime \
  --runtime target/release/wbuild/vara-runtime/vara_runtime.wasm \
  on-runtime-upgrade --blocktime 3000 --checks all --disable-mbm-checks \
  snap -p /tmp/vara-mainnet-11000-a83455.snap \
  > /tmp/vara-mainnet-full-wasm-final.log 2>&1
~~~

All configured pallet try-state checks ran; the second upgrade retained root `0xdc9c2db9e8afa471bfe7dd39e9f1c98f29d09380f2ec0d1e56ca961551c37843`. Reported usage was **0.501625 s / 50.16%**, compressed PoV **157.0 KiB**, with no weight safety issues. Historical staking exposure warnings are retained in the log. Preserve the matching try-runtime artifact before any non-try-runtime build replaces the shared `wbuild` path. Production-profile approval, reference-hardware calibration and normal-timing operational rehearsals remain distinct release gates.

### Source-chain physical rehearsal

`scripts/beefy-activation-smoke.mjs` passed on two isolated, indexed native nodes with explicit four-slot dev timing. Activation occurred at block **31**; **eight** cryptographically verified native BEEFY commitments covered the initial two-member set, both rotated ECDSA keys, and the one-member set after purging the last validator. BABE/GRANDPA/BEEFY ownership APIs admitted current owners and rejected the purged owner; GRANDPA/BEEFY rejected wrong set IDs. The same test exposed and fixed Operational weight classification and raw-SURI/opaque-response handling before it passed. Each transition retains the 120-second timeout. Both nodes were stopped afterward.

Evidence JSON SHA-256: `0edcd0026ddfd6b0903aba58352f5c0b5904bbecd549ae47571220fabfaa8236`. Normal non-dev/non-try-runtime `--release` build also passed; compressed WASM SHA-256 `0d0b7b4c386404a6621dd2da811d30fafe108114487160038bbf1b79332a0c35`. Generated normal-dev gsdk metadata was refreshed and `cargo check --release --locked -p gsdk` passed. These are not production-profile approval, a normal two-hour-era rehearsal, or an Ethereum real-fund cutover.


## 3. Run local Gear + Anvil end to end

Do not start separate nodes or Anvil for this command. The runner owns its processes, chooses loopback ports and uses a fresh source database.

~~~sh
cd "$BRIDGE_DIR"
export RUN_DIR="/tmp/vara-beefy-secure-v1-rehearsal-$(rtk date +%s)"
rtk target/release/beefy-relay rehearse \
  --gear-node "$GEAR_DIR/target/beefy-e2e/release/gear" \
  --output-dir "$RUN_DIR"
rtk jq '{status,preparationElapsedSeconds,liveElapsedSeconds,sourceGenesis,mmrStartBlock,beefyActivationBlock,bootstrapBlock:.bootstrap.block,initialClientRoot:.ethereum.checkpoint.root}' "$RUN_DIR/manifest.json"
~~~

`RUN_DIR` must not already exist. Pick another name on collision; do not delete previous evidence merely to rerun.

The command performs:

1. Start both indexed archive authorities before waiting for finalized source state. Alice alone cannot finalize this two-authority local chain.
2. Subscribe to BEEFY and authenticate a finalized checkpoint `C>A`, including the newest leaf's real parent timestamp. Cross-check the same checkpoint with Bob.
3. Deploy the client, queue-bound adapter, queue and test receiver. Verify the exact bootstrap, policy and bindings. The initial client root remains zero.
4. Continue the same source stream after `C`, including handovers produced during deployment.
5. Within the separate 120-second live deadline, rotate the actual BEEFY key twice, send two real messages, verify maturity/replay behavior, retain a proof across natural clear, reject its stale anchor and rebuild it for successful delivery.

Preparation also has its own 120-second deadline. The local runner advances Anvil time for the two message-maturity waits. It never invents source timestamps.

### Inspect the outcome

Require exit status zero and `manifest.status == "passed"`. The runner captures each message proof before finality/queue clear, then checks that the finalized inclusion hash matches. A fresh documentation-validation run on 2026-09-21 aborted with `message reorged after its proof was retained`; it had delivered the first message, not the second. This is a fail-closed abort, not successful end-to-end evidence. The bounded runner does not automatically reconcile that reorg.

Preserve the failed evidence. For this disposable local command only, a retry can use a new output directory and a fresh owned chain. Do not weaken the hash comparison or apply fresh-chain retries to a persistent Hoodi deployment. Durable canonical-inclusion/proof reconciliation is a public-network release gate.

The retained files are:

- `manifest.json`: schema version 2, source genesis/`A`/`G`, bootstrap proof and authority sets, immutable destination bindings, policy, artifact hashes and separate elapsed times.
- `commitments.jsonl`: native signed commitments, newest freshness witnesses and reconciled accepted source timestamps.
- `messages.json`: both messages, queue inclusion data and delivery/replay/stale-anchor outcomes.
- `transactions.json`: receipts, gas and checked events.
- Authority process logs: diagnose source startup, finality or RPC failures.

~~~sh
rtk jq 'map({sourceBlock,delivered,earlyDeliveryRejected,replayRejected,staleAnchorRejected})' "$RUN_DIR/messages.json"
rtk jq -s 'map({block,finalizedHeight,accepted,acceptedSourceTimestampMs})' "$RUN_DIR/commitments.jsonl"
~~~

A null rejection field means that particular check was not attached to that message; the first message exercises early delivery, both exercise replay, and the retained second message exercises the stale-anchor race.

Owned processes and temporary node data are cleaned up on success or failure. The source database and rotated private keys are not retained with the evidence. This is a completed demonstration, **not a persistent network for a later Hoodi deployment**. Preserve the evidence externally if needed; `/tmp` is not durable storage.

## 4. Prepare persistent local Gear for a Hoodi demonstration

The commands in this section start only the source network. They do not deploy a BEEFY bridge to Hoodi.

**Alice/Bob development keys are public. Anyone can sign as these validators. Contracts bootstrapped from this source are suitable only for a valueless demonstration, even if deployed on a public Ethereum testnet.** Use a separately configured chain with private operational validator keys for a security-representative test. Never connect either setup to production assets or existing production bridge contracts.

Use the built node from section 2. In each terminal, set `GEAR_DIR` as above and choose a dedicated persistent directory:

~~~sh
export DEMO_DATA="$HOME/.local/share/vara-beefy-hoodi-demo"
~~~

Do not reuse an unrelated database. Check that RPC ports 9944/9945 and P2P ports 30333/30334 are free, or change them consistently. The following RPCs and P2P listeners are loopback-only. Keep unsafe authoring RPC local; never add `--rpc-external` for convenience.

Terminal A, leave running:

~~~sh
rtk "$GEAR_DIR/target/beefy-e2e/release/gear" \
  --chain local --alice --validator --force-authoring \
  --unsafe-force-node-key-generation \
  --base-path "$DEMO_DATA/alice" \
  --rpc-port 9944 --rpc-methods unsafe \
  --listen-addr /ip4/127.0.0.1/tcp/30333 \
  --enable-offchain-indexing true \
  --state-pruning archive --blocks-pruning archive \
  --no-mdns --no-telemetry --no-prometheus
~~~

Terminal B, after Alice's RPC is listening, obtain its peer ID and start Bob immediately. Do not wait for Alice to finalize before starting Bob:

~~~sh
export ALICE_PEER="$(rtk proxy curl --fail --silent --show-error \
  -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"system_localPeerId","params":[]}' \
  http://127.0.0.1:9944 | rtk jq -er '.result | select(type == "string" and length > 0)')"
: "${ALICE_PEER:?Alice RPC did not return a peer ID}"
rtk "$GEAR_DIR/target/beefy-e2e/release/gear" \
  --chain local --bob --validator --force-authoring \
  --unsafe-force-node-key-generation \
  --base-path "$DEMO_DATA/bob" \
  --rpc-port 9945 --rpc-methods unsafe \
  --listen-addr /ip4/127.0.0.1/tcp/30334 \
  --bootnodes "/ip4/127.0.0.1/tcp/30333/p2p/$ALICE_PEER" \
  --enable-offchain-indexing true \
  --state-pruning archive --blocks-pruning archive \
  --no-mdns --no-telemetry --no-prometheus
~~~

Check both node logs for successful peering. In a third terminal, query each finalized head and repeat later to confirm advancement:

~~~sh
rtk proxy curl --fail --silent --show-error -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"chain_getFinalizedHead","params":[]}' http://127.0.0.1:9944
rtk proxy curl --fail --silent --show-error -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"chain_getFinalizedHead","params":[]}' http://127.0.0.1:9945
~~~

Two different latest heads may indicate ordinary lag. The bootstrap collector must compare the **same finalized checkpoint hash** on both nodes, not equate two unrelated latest responses. Preserve both databases across restarts. If the source chain is reset, do not resume against the old Hoodi client/queue; treat it as a separate deployment and reconcile/retire the old demonstration first. Local chain specs can reuse a genesis while their later histories differ.

## 5. Hoodi network preflight

The [Hoodi network metadata](https://github.com/eth-clients/hoodi) specifies execution chain ID **560048** and genesis hash:

~~~text
0xbbe312868b376a3001692a646dd2d7d1e4406380dfd86b98aa8a34d1557c971b
~~~

Use a provider endpoint for that network and a dedicated test deployer with Hoodi ETH. Obtain test funds through resources linked from the [Hoodi homepage](https://hoodi.ethpandaops.io/). Official metadata recommends Sepolia for general application tests; Hoodi is the network chosen here for the requested infrastructure exercise.

Supply `HOODI_RPC_URL` and the public `DEPLOYER_ADDRESS` in your shell. Do not paste private keys into documentation, command histories or evidence. These commands are read-only:

~~~sh
: "${HOODI_RPC_URL:?Set a Hoodi execution RPC URL}"
: "${DEPLOYER_ADDRESS:?Set the public address of the dedicated test deployer}"
CHAIN_ID="$(rtk proxy cast chain-id --rpc-url "$HOODI_RPC_URL")"
test "$CHAIN_ID" = 560048
rtk cast block 0 --rpc-url "$HOODI_RPC_URL" --json
rtk cast balance "$DEPLOYER_ADDRESS" --rpc-url "$HOODI_RPC_URL" --ether
~~~

Verify the returned genesis hash against the value above. Keep the source host's clock synchronized: source timestamps are checked against real Ethereum block time. There are no public-network equivalents of `vm.warp`, `anvil_increaseTime` or `vm.roll`.

## 6. Required implementation before broadcasting BEEFY contracts on Hoodi

**Stop here for an end-to-end Hoodi deployment with the current code.** No `BeefyHoodi` script, external-network relay command or checkpoint-export command exists in this revision. The items below are concrete prerequisites, not commands that can already be run.

| Missing capability | Required behavior and existing code to reuse |
| --- | --- |
| Persistent-source bootstrap collector | Accept the two source RPC endpoints; reuse `source.rs` to subscribe, validate finalized native commitments, authenticate the newest proof and compare the same checkpoint on both nodes. Export the validated checkpoint and retain the stream/cursor; never derive it from synthetic fixtures. |
| Explicit Hoodi BEEFY deployment entrypoint | Require chain 560048; deploy `BeefyClient` from that checkpoint; reuse `Base.deployBridge` and its predicted `messageQueueAddress` hook to deploy the immutable adapter. Take real test governance/program IDs and emergency roles from reviewed configuration, not `BaseConstants`. |
| External Ethereum relay | Replace Anvil ownership with an explicit RPC/signer/contract configuration. Reuse native signature validation, newest-leaf submission, queue-proof generation and on-chain checkpoint reconciliation. Add durable restart, transaction/finality and nonce handling. |
| Public-network scenario driver | Send real source messages and exercise rotations/clear/replay/stale-anchor rebuilding while waiting for actual destination maturity and finality. Keep the existing local runner's 120-second deadlines unchanged; a public-network operator workflow is a separate execution mode. |
| Application/governance integration | For more than a mock receiver, deploy/configure the Gear governance and VFT programs, Ethereum application contracts, token mappings and the Ethereum-to-Gear services for Hoodi. Verify their permissions and asset accounting independently of BEEFY. |

Do not just remove `BeefyLocal`'s chain check. It contains development governance identifiers and mock configuration. Do not use the generic `DeploymentScript` as a shortcut: it will deploy the legacy verifier rather than the BEEFY adapter.

### Exact bootstrap input contract

The existing local deployment already uses these ten values. The reviewed Hoodi deployment must use the same validated meanings:

| Input | Value from the authenticated source checkpoint |
| --- | --- |
| `BEEFY_SOURCE_GENESIS` | Nonzero source block-0 hash, not Ethereum's genesis |
| `BEEFY_MMR_START_BLOCK` | Discovered first MMR insertion block `A`, not an assumed activation height |
| `BEEFY_INITIAL_BLOCK` | Finalized signed checkpoint `C`, with `A<C<=u32::MAX` |
| `BEEFY_INITIAL_SOURCE_TIMESTAMP_MS` | Real `Timestamp.Now` at parent `C-1`, matching the newest MMR leaf |
| `BEEFY_CURRENT_ID` | Current native authority-set ID, fitting u64 |
| `BEEFY_CURRENT_LENGTH` | Exact current ordered authority count, 1..256 |
| `BEEFY_CURRENT_ROOT` | Current nonzero native authority Merkle root |
| `BEEFY_NEXT_ID` | Next native authority-set ID, current ID + 1, fitting u64 |
| `BEEFY_NEXT_LENGTH` | Exact next authority count, 1..256 |
| `BEEFY_NEXT_ROOT` | Next nonzero native authority Merkle root |

Record genesis, `A`, BEEFY activation `G`, `C`, block hash, finalized height, both authority key lists/roots and the newest native proof. Validate `snapshot.hash()==leaf.leaf_extra`. Initial source time must be no older than 24 hours and no more than 120 seconds ahead at Ethereum deployment time.

Do not copy bootstrap values from the disposable local rehearsal: its source processes and databases have been destroyed. A fresh source with the same development genesis is not evidence of the same later chain history.

## 7. Hoodi execution sequence after those capabilities ship

Use only the reviewed deployment/relay commands supplied by that future release. No fictitious CLI flags or broadcast command are provided here.

1. Keep the persistent source authorities running and verify their finality, source identity, MMR history and actual current/next keys. Stop if the source is unhealthy or its time differs excessively from Ethereum.
2. Capture and independently authenticate the finalized bootstrap described above. Retain the same subscription/catch-up state while contracts deploy.
3. Verify the Hoodi chain/genesis and deployer funding. Simulate the complete deployment before broadcasting; review all governance, emergency and application identities.
4. Deploy the client, governance/application dependencies, queue-bound adapter and queue in the reviewed Base ordering. Avoid concurrent deployment transactions from the same account that could invalidate address predictions. Never predict a queue using a newly invented nonce offset.
5. Check deployed bytecode, constructor values, queue/client/chain bindings and the initial **zero** accepted root. Fresh liveness alone is insufficient to register roots.
6. Start the external relay strictly after bootstrap `C`. Submit every required handover before later commitments. Require a later accepted nonzero MMR root and exact authenticated source timestamp.
7. Send a valueless source message, retain its historical queue proof, register through `MessageQueue.submitMerkleRoot`, and await an included successful receipt and the selected destination finality policy. Do not call the adapter directly as a registration preflight.
8. Wait the real queue maturity interval. Current user-message maturity is five minutes, pauser-message maturity five minutes and admin-message maturity one hour. A public-network test cannot compress these with cheatcodes.
9. Process the message and verify its destination event and processed nonce. Reject replay. Repeat with a retained message across two actual authority-key rotations and natural queue clear; reject the stale anchor, regenerate its proof against the latest accepted root and deliver.
10. Restart the relay without resetting either chain and verify recovery from its durable state. Archive receipts, source proofs, bindings and accepted timestamps. Leave the source and relay running while the deployment is in use, or retire it explicitly.

Fiat-Shamir submissions avoid the separate interactive RANDAO wait. If testing the interactive path, keep the fixed 128-block delay and 24-block window; do not assume the local test's instantaneous block rolls are available on Hoodi. Interactive final-call gas in the migration guide excludes the other two transactions.

### Read-only post-deployment checks

After the reviewed deployment exists, set `BEEFY_CLIENT_ADDRESS`, `BEEFY_ADAPTER_ADDRESS` and `MESSAGE_QUEUE_ADDRESS` from its receipts, then query:

~~~sh
rtk cast call "$BEEFY_CLIENT_ADDRESS" 'sourceGenesis()(bytes32)' --rpc-url "$HOODI_RPC_URL"
rtk cast call "$BEEFY_CLIENT_ADDRESS" 'mmrStartBlock()(uint64)' --rpc-url "$HOODI_RPC_URL"
rtk cast call "$BEEFY_CLIENT_ADDRESS" 'latestBeefyBlock()(uint64)' --rpc-url "$HOODI_RPC_URL"
rtk cast call "$BEEFY_CLIENT_ADDRESS" 'latestMMRRoot()(bytes32)' --rpc-url "$HOODI_RPC_URL"
rtk cast call "$BEEFY_CLIENT_ADDRESS" 'lastAuthenticatedSourceTimestampMs()(uint64)' --rpc-url "$HOODI_RPC_URL"
rtk cast call "$BEEFY_CLIENT_ADDRESS" 'isLive()(bool)' --rpc-url "$HOODI_RPC_URL"
rtk cast call "$BEEFY_ADAPTER_ADDRESS" 'beefyClient()(address)' --rpc-url "$HOODI_RPC_URL"
rtk cast call "$BEEFY_ADAPTER_ADDRESS" 'messageQueue()(address)' --rpc-url "$HOODI_RPC_URL"
rtk cast call "$BEEFY_ADAPTER_ADDRESS" 'destinationChainId()(uint256)' --rpc-url "$HOODI_RPC_URL"
rtk cast call "$MESSAGE_QUEUE_ADDRESS" 'verifier()(address)' --rpc-url "$HOODI_RPC_URL"
~~~

Require the intended source genesis and `A`, the intended client/queue/adapter addresses, destination 560048, matching checkpoint/time and fresh trust. Check current/next authority tuples and all compiled policy constants against the deployment record as well. Require a nonzero root only after the later signed update, not immediately after construction.

## Troubleshooting and stop conditions

| Symptom | Action |
| --- | --- |
| Finalized height remains zero | Start both authorities, confirm peering and matching chain data. Do not weaken finality or bootstrap from best-chain state. |
| Newest snapshot hash differs from the native leaf | Check the historical parent timestamp/state, `A`, initialization event and runtime version. Fail closed; do not try arbitrary queue IDs or synthetic times. |
| `BeefyLocal requires chain id 31337` | Expected on Hoodi. Use the missing-capability gates, not a patched chain guard. |
| Upgrade validation reports duplicate/stale build-info | Clean generated Foundry outputs and perform a full forced rebuild. Keep upgrade validation enabled. |
| Adapter returns false / queue reports `InvalidPlonkProof` | Check queue caller and chain binding, v1 canonical bytes, latest accepted anchor, initialized nonzero queue root, source coordinates and client liveness. |
| Root proof became stale | Regenerate the historical leaf proof under the current accepted anchor; retain mandatory authority handovers. |
| Client expired | Stop new registrations and enter the approved recovery procedure. A fresh incoming leaf, destination clock change or process restart cannot renew expired trust. |
| Message reorged after its proof was retained | Initial inclusion and finalized inclusion differ. Preserve the failed evidence; do not deliver the old proof or disable the guard. Only the disposable local runner may be retried on a fresh owned chain. Persistent operation needs canonical transaction/proof reconciliation. |
| Source database was reset | Do not reuse its existing destination trust state. Retire/reconcile the old deployment and perform a new isolated bootstrap. |
| Existing queue cannot change verifier | Expected: no public setter exists. Implement and approve the storage-preserving governance upgrade/recovery gate first. |

Bridge-repository code references (`gear-tech/gear-bridges`, not this Gear checkout): `tools/beefy-relay/src/{main.rs,source.rs,rehearsal.rs,ethereum.rs}`, `ethereum/script/{BeefyLocal.s.sol,Deployment.s.sol}`, `ethereum/test/Base.sol` and `ethereum/src/MessageQueue.sol`. Resolve these against the reviewed Bridge revision in the release manifest; the historical publication inventory is not a current deployment pin.
