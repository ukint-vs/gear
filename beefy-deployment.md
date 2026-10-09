# Mainnet BEEFY deployment and migration

## Mainnet operator runbook

This runbook is for the **mainnet 11000 → 20100** four-key migration, preserving runtime identity `vara` and the existing chain. **Installing the node, upgrading the runtime, activating source BEEFY, and cutting over the Ethereum bridge are four separate actions.** Source activation requires neither a destination binding nor bridge approval. Publishing a release performs none of these actions. Start with the [coordinator checklist](beefy-migration.md#mainnet-coordinator-checklist); validators follow the [node checklist](vara/node/README.md#beefy-upgrade-and-later-activation).

The selected rollout is mainnet only. The retained mainnet snapshot qualifies the shared migration, with separate `dev` feature tests/build checks; a public-testnet deployment is not a prerequisite. Never substitute testnet keys, genesis hashes, runtime artifacts, bindings or checkpoints. An unsupported mainnet predecessor is a stop, not permission to edit storage. This document does not authorize transactions.

### Release ownership and remaining gates

| Owner | Required evidence |
| --- | --- |
| Release coordinator (you) | Reviewed immutable commit, mainnet node/runtime/metadata artifact SHA-256 checksums, chain spec/genesis, finalized supported predecessor, spec/transaction/API versions, normal timing/features, archive endpoints and separate upgrade/activation/cutover approvals. Publish through the existing release pipeline. |
| Runtime maintainers | Reuse the supported full mainnet snapshot for the shared migration implementation, preserving its finalized input root and migration/idempotence/decoding/try-state/weight evidence. Check the `dev` variant separately through tests and network-specific build/metadata/import checks; retain per-network state inventory, exact-artifact staging and baseline-host capacity qualification. |
| Validators / coordinator | Compatible nodes installed before enactment, protected persistent keystores, finalized native registration and actual active/queued propagation; readiness of electable standby operators, not mandatory return of all dormant owners. |
| Governance | Separately approved network runtime upgrade and source activation; successful finalized inner dispatch, not merely an included wrapper. Bridge pause/binding/unpause have separate approvals. |
| Bridge operators / reviewers | Reviewed v2-compatible destination/relay, deployed configuration/bootstrap verification, legacy drain/replay/custody reconciliation and authorized recovery before opening traffic. |

Production-profile artifacts are a [release-workflow](.github/workflows/release.yml) responsibility, not a validator build task. The workflow checks out its `tag_name` input: create the approved tag on the final merged commit before dispatching it. Prefer a draft with `make_latest=false`, verify the artifacts/checksums, then publish deliberately. Publication is not runtime enactment. `2_01_00` means on-chain spec **20100**; verify actual published filenames.

Use the published `gear` node with **`--chain vara`**, `production_vara_runtime_v2_01_00.wasm` and `production_vara_runtime_v2_01_00_metadata.scale`, checked against `SHA256SUMS`. The runtime must exclude `dev`, `fast-runtime`, `try-runtime` and `runtime-benchmarks`. Do not upload `testnet_vara_runtime_v*.wasm` or a try-runtime companion.

Inspect the approved release tag through the release API; this lists artifacts,
not an approval to use a different or latest release. Verify downloaded node/WASM
files against the approved mainnet checksum manifest. API asset digests, where
present, do not replace the reviewed network-specific manifest.

~~~sh
: "${RELEASE_TAG:?Set the approved immutable release tag}"
curl --fail --silent --show-error \
  "https://api.github.com/repos/gear-tech/gear/releases/tags/$RELEASE_TAG" \
  | jq -e '{tag: .tag_name, target: .target_commitish, assets: [.assets[] | {name, digest, url: .browser_download_url}]}'
: "${NODE_ARTIFACT:?Set the downloaded node artifact}" "${NODE_SHA256:?Set its approved SHA-256}"
printf '%s  %s\n' "$NODE_SHA256" "$NODE_ARTIFACT" | sha256sum --check -
: "${RUNTIME_ARTIFACT:?Set the correct network WASM}" "${RUNTIME_SHA256:?Set its approved SHA-256}"
printf '%s  %s\n' "$RUNTIME_SHA256" "$RUNTIME_ARTIFACT" | sha256sum --check -
~~~

Mainnet uses normal three-second slots, two-hour sessions and six-session/twelve-hour eras. The release node intentionally embeds the testnet runtime; mainnet nodes execute the actual on-chain mainnet WASM, selected with `--chain vara`. Use a same-revision mainnet try-runtime companion only for migration APIs, never as upgrade WASM. Byte identity across profiles is not assumed.

### 1. Coordinator: qualify the supported predecessor

Pin finalized state. Inventory all `Session.NextKeys` owners, `Session.Validators`, `Session.QueuedKeys`, four-key ownership, historical roots/exposures and bridge state. Check exact spec **11000**, matching network name and four-key encoding. Reuse an existing compatible full snapshot with its original block/hash/root; rerun the changed runtime against it. Archive inputs and new logs/proof-retention evidence durably; ephemeral paths are not release archives.

Testnet uses the same runtime with `dev` enabled. The native session-key migration and BEEFY/bridge readiness implementation are shared, so the retained mainnet snapshot rehearsal supplies their full-state migration evidence; a separate public-testnet snapshot is not an implementation acceptance gate. Existing `dev` tests and normal-timing testnet artifact checks cover the feature variant. This does not equate chain state or approvals: `dev` includes Sudo and testnet identity, while the mainnet migration tuple additionally includes the builtin ED-lock migration. Keep network-specific inventories and deployment qualification separate.

Using pinned try-runtime CLI **0.10.1**:

~~~sh
: "${SNAPSHOT:?Select the retained supported mainnet snapshot}" "${TRY_RUNTIME_WASM:?Select the matching mainnet companion}"
test -f "$SNAPSHOT" || exit 1
try-runtime --runtime "$TRY_RUNTIME_WASM" on-runtime-upgrade \
  --blocktime 3000 --checks all --disable-mbm-checks snap -p "$SNAPSHOT"
~~~

Match the unmodified input root to the finalized header. Keep spec, decoding, idempotence and unsuppressed weight checks. Only multi-block simulation is disabled: this runtime has no multi-block migrator and that simulation fabricates the predecessor version. Oversized state must fail qualification. Scoped externalities or fresh-chain smoke results do not prove an existing-network upgrade or live finality.

### 2. Validators and archives: install before runtime upgrade

Verify release node checksums/version and coordinate rolling restart without losing BABE/GRANDPA quorum. Preserve service account, chain spec, database/base path, network identity, keystore path and password configuration. Do not purge/resync, reset genesis or run concurrent signing nodes with the same keys.

**ABI warning:** old nodes are incompatible with the changed SessionKeys v2 runtime ABI. The new node's API v1 fallback only supports the four-key predecessor **before upgrade**; it does not let old nodes run the new runtime. Do not register five-key/v2 bundles on the predecessor.

Back up/protect the **entire existing keystore** and password under established custody procedures. Retain all old private entries through active/queued handover and offence-proof/recovery retention. Keep unsafe authoring RPC loopback-only/access controlled; retain validator mode, GRANDPA and BEEFY networking.

Enable `--enable-offchain-indexing true` **before upgrade/first MMR insertion**. Arrange at least two independent proof servers with `--state-pruning archive --blocks-pruning archive`, indexed from the first insertion. Validators need not all become archives. Flags cannot restore pruned state or backfill offchain MMR nodes; late servers need verified replay/recovery. Compare the same finalized hashes and exercise historical proof serving, not just uptime.

### 3. Governance: enact and inspect the upgrade

Through the existing mainnet governance route, obtain Root execution of `system.setCode` with the approved production WASM; mainnet has no Sudo shortcut. After execution, verify finalized code/checksum, unchanged `vara` identity, spec **20100**, metadata/API versions and successful inner dispatch. Keep BABE/GRANDPA and the legacy bridge operating.

Migration preserves four public-key fields, validator/queued ordering and key ownership; appends deterministic placeholder `0x02 || Keccak256(stash.raw32)`; preserves **bridge pause state, queue, nonce, owners and history**; and does not automatically pause, bind, clear or reset the bridge. Older historical roots stay unchanged; original current/queued roots remain usable for issued ownership proofs while their sessions are retained, alongside rebuilt five-key roots. Missing inactive BEEFY bookkeeping is initialized without replacing existing records or activating BEEFY. First-upgrade `Beefy.GenesisBlock` remains absent. Placeholders are not operational signers.

At common finalized state, use approved metadata-aware query tooling to read `Staking.Bonded(stash)` and determine the **effective signed account** executing Session: distinct controller where applicable, or represented proxy/multisig account, not the outer fee payer. Session stores keys under the converted validator/stash. Reconcile absent/changed bonding; never guess the signer.

Read-only HTTP JSON-RPC examples (Bash, curl and jq):

~~~sh
: "${SOURCE_HTTP:?Set the approved mainnet HTTP RPC}"
SPEC_NAME=vara
FINALIZED_HASH="$(curl --fail --silent --show-error -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"chain_getFinalizedHead","params":[]}' \
  "$SOURCE_HTTP" | jq -er '.result | select(test("^0x[0-9a-fA-F]{64}$"))')"
curl --fail --silent --show-error -H 'Content-Type: application/json' \
  --data "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"state_getRuntimeVersion\",\"params\":[\"$FINALIZED_HASH\"]}" \
  "$SOURCE_HTTP" | jq -e --arg spec "$SPEC_NAME" \
  '.result | select(.specName == $spec and .specVersion == 20100) | select(any(.apis[]; .[0] == "0xab3c0572291feb8b" and .[1] == 2))'
curl --fail --silent --show-error -H 'Content-Type: application/json' \
  --data "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"state_getMetadata\",\"params\":[\"$FINALIZED_HASH\"]}" \
  "$SOURCE_HTTP" | jq -er '.result | select(type == "string" and length > 2)'
~~~

The SessionKeys API ID **`0xab3c0572291feb8b`** is derived by the pinned SDK runtime-API macro: eight-byte Blake2b hash of `SessionKeys`. Require advertised API **2**, not merely spec 20100. Decode metadata in the approved query/signing tool. Also query `chain_getBlockHash` with `[0]` and compare actual genesis to the manifest.

### 4. Validators: native generation and ordinary registration

After finalized API v2 is live, use the released node's local unsafe **`author_rotateKeysWithOwner`**. Set `OWNER` to the actual effective signer's **raw 32-byte AccountId32 hex**: not SS58 text or a length-prefixed SCALE Vec. Production generation uses the configured keystore and **seed None**, not a deterministic development seed.

~~~sh
: "${OWNER:?Set the actual effective raw AccountId32 hex}"
[[ "$OWNER" =~ ^0x[[:xdigit:]]{64}$ ]] || exit 1
NATIVE_KEYS="$(curl --fail --silent --show-error -H 'Content-Type: application/json' \
  --data "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"author_rotateKeysWithOwner\",\"params\":[\"$OWNER\"]}" \
  http://127.0.0.1:9944 | jq -e '.result | select(.keys | type == "string" and test("^0x[0-9a-fA-F]{322}$")) | select(.proof | type == "string" and test("^0x[0-9a-fA-F]{642}$"))')"
KEYS="$(printf '%s' "$NATIVE_KEYS" | jq -er '.keys')"
PROOF="$(printf '%s' "$NATIVE_KEYS" | jq -er '.proof')"
curl --fail --silent --show-error -H 'Content-Type: application/json' \
  --data "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"author_hasSessionKeys\",\"params\":[\"$KEYS\"]}" \
  http://127.0.0.1:9944 | jq -e '.result == true'
~~~

Use the actual service port. Require **161 public-key bytes** (BABE/GRANDPA/ImOnline/AuthorityDiscovery/BEEFY order) and **321 native proof bytes**, the five-signature tuple. Every key signs `POP_ || owner` with its application scheme. ECDSA possession uses normal Blake2-based signing, not Keccak/prehashed BEEFY commitment signing; noncanonical high-S possession is rejected. Native proofs are owner-bound, not genesis-/whole-bundle-bound. Never use `author_rotateKeys` plus `0x` proof, concatenate keys manually or export session secrets.

This rotates **all five keys**, not only BEEFY. Migration's preservation of four public fields does not mean later native rotation preserves them. Keep all old private entries through duty and retention periods. In an approved metadata-aware ordinary extrinsic interface, submit **`session.setKeys(keys, proof)`** from the effective owner, with liquid unbonded funds for fees; use the generic interface if a wizard sends empty proofs. For proxy/multisig check the represented origin. Verify finalized dispatch, including wrapper results, and exact `Session.NextKeys(stash)` equality.

The operator's approved transaction tool can produce an ordinary signed extrinsic for RPC transport (this command neither signs nor checks finalization):

~~~sh
: "${SIGNED_EXTRINSIC:?Set the approved signed session.setKeys extrinsic}"
[[ "$SIGNED_EXTRINSIC" =~ ^0x[[:xdigit:]]+$ ]] || exit 1
curl --fail --silent --show-error -H 'Content-Type: application/json' \
  --data "{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"author_submitExtrinsic\",\"params\":[\"$SIGNED_EXTRINSIC\"]}" \
  http://127.0.0.1:9944 | jq -er '.result'
~~~

A hash is not successful finalized registration. Retain finalized receipt and public bundle/proof, never secrets. No separate key signer, code download or manually built bundle is required.

**Keystore contract:** restart with the same custom `--keystore-path` and password configuration. Password participates in private-key derivation; it is **not keystore-file encryption**. Protect OS permissions and backups separately. Presence RPCs do not prove signing. Proof/signing failure fails generation, not a partial/empty-proof success. Five-key generation is **nontransactional**: unused new entries may remain after failure. Preserve existing keys and fix configuration/error before retrying; never delete a keystore as recovery.

### 5. Coordinator: verify actual source readiness

At one finalized hash compare ordered `Historical.ActiveSessionKeys` / `Session.QueuedKeys` to `Beefy.Authorities` / `Beefy.NextAuthorities`. Require nonempty unique validators and valid non-placeholder unique BEEFY keys, correct `Beefy.SetIdSession`, exact current/next MMR authority IDs/lengths/Merkle commitments and initialized nonzero MMR history.

Source activation allows **1..1000 actual active and queued authorities**, independently of binding, desired `Staking.ValidatorCount` or dormant registrations. Active, queued and **electable standby** operators prepare native keys. An unready electable operator chills through existing staking before selection. There is no readiness election filter, forced chilling or guarantee of future private-key availability. Dormant non-electable owners need not return/purge. No pending-owner counter or custom proof registry gates activation.

Observe actual propagation through normal sessions and an era transition, not a fixed sleep. Registration commonly reaches the session after next, subject to selection/inclusion timing. Preserve archive/handover evidence before pruning. Ordinary purge is neither unbonding nor private-key deletion; coordinate retirement without using it as an activation-counter shortcut.

### 6. Governance: schedule source BEEFY independently

After source readiness/approval, call **`beefy.setNewGenesis(delayInBlocks)`** through approved Root governance, with positive checked future target. **G = execution block + delay**, not proposal block + delay. It is Operational with a separate source **1000-authority reservation**; include real governance-wrapper admission. The argument-aware origin enforces readiness even through Root/bypass dispatch.

No destination binding/cutover is required. Activation/restart changes BEEFY start, not public-chain genesis, MMR, bridge pause/queue/nonce or custody. Verify successful finalized dispatch and `Beefy.GenesisBlock`; after G require advancing cryptographically valid native quorum commitments, handovers and historical MMR proofs across independent nodes. First MMR insertion **A** (upgrade) need not equal BEEFY start **G**.

### 7. Bridge operators: separately pause, drain, bind and enable

Keep legacy GRANDPA traffic operating until its independently approved cutover:

1. **Pause/drain/reconcile:** use the existing approved source/destination freeze and cutoff procedure; stop legacy writers at the reconciled cutoff, inventory roots/messages/consumed nonces/custody and preserve authenticated deliveries. Source pausing does not invalidate already authenticated legacy destination proofs. Enforce destination/application cutoff separately; do not assume source pause drains it or reset source queue/nonce/history.
2. **Verify deployment/capacity:** qualify paired v2 contracts/relay, deployed bytecode, governance and immutable queue/client/chain configuration. Actual current **and queued**, and desired `Staking.ValidatorCount`, must each fit **256**. This is bridge policy, not the source 1000 bound; >256 source BEEFY can activate while bridge traffic remains disabled.
3. **Bind once while paused:** approved Root calls `gearEthBridge.bindDestination(chainId, queue)`. Chain ID is nonzero **32-byte big-endian H256** and queue the approved nonzero 20-byte address. It captures actual nonzero `System.BlockHash(0)` and derives `Keccak256("vara/gear-eth-bridge-domain/v2" || sourceGenesis[32] || chainIdBE[32] || queue[20])`. Verify finalized `DestinationBound`, stored tuple and domain. Binding may happen **before or after BEEFY activation**, only paused/unbound; no rebinding or raw-storage repair.
4. **Authenticate post-binding readiness:** obtain a real signed **post-binding** newest leaf and v2 snapshot under that domain, compare one finalized checkpoint independently, initialize/verify destination client, continue required handovers and accept a later authenticated nonzero root. A pre-binding leaf or domain write is not destination-readiness evidence.
5. **Explicitly unpause:** after destination/replay/custody/canary approval, call `gearEthBridge.unpause` through its authorized governance route and check finalized dispatch. It remains **Normal**, with a separate full-bridge **256-authority allowance**, not source Operational/1000 reservation. Verify matured canary/replay rejection before assets. Maintain Ethereum-to-Gear infrastructure.

Preserve or verifiably migrate consumed-nonce/replay state and custody. Message hashing is unchanged; an empty new queue can replay old consumed messages. Do not run two unrestricted queues for the same assets. MessageQueue has no public verifier setter: a storage-preserving switch or fresh-queue migration needs its reviewed authorized mechanism, not another initialize or invented setter. Recovery must remain reachable when the old verifier expires.

### 8. Later rotations, elections and restarts

Monitor actual active/queued suitability, desired count, native finality, archives, destination freshness and progress. Bound admission fails closed on malformed/missing identity, absent/not-yet-live BEEFY, invalid MMR/descriptors/session mapping or actual/desired >256. Scheduling a later future genesis blocks bound admission **immediately**, including that block. Wait for the new start and verify new signatures/destination readiness. Structural admission cannot guarantee future private signing.

There is **no automatic pause-bit change or queue/destination reset** on readiness failure. Explicitly pause/reconcile when safety requires. Lowering desired count does not shrink oversized actual/queued sets; same-committee sessions can retain them. Do not assume recovery from actual >256: observe a suitable real handover and exact commitments. Per-message admission checks structural/capacity state rather than the full cryptographic scan at unpause.

BEEFY-only rotations preserve the queue; actual GRANDPA changes retain delayed rollover. Pending clear rejects every enqueue path, including governance, with `BridgeCleanupRequired`, preserving queue/nonce. Keep historical root/proof evidence. Existing overflow-reset finalization/GRANDPA-proof rules remain, not activation shortcuts. Readiness rejections preserve fees/message state. Old binaries, repeated scheduling, wiped MMR or fresh queues are not rollback.

## Quorum and destination sampling

Native quorum is **N − floor((N−1)/3)**; **N ≤ 3 requires unanimity**. A process or one signature is not quorum evidence.

Destination claimed native quorum and selected-signature verification are distinct. Existing paired policy uses a **floor(N/3)+1 selection cap and fixed 86/86 floors**, yielding 20 selected at N=59, 51 at 150 and 86 at 256. Confirm Fiat-Shamir/interactive constants in the reviewed artifact; do not use a blanket one-third quorum formula. Interactive RANDAO delay/window remain 128/24 destination blocks.

## Hardware and weight qualification

The [published validator baseline](https://wiki.vara.network/docs/vara-network/staking/validate#hardware-requirements) remains **2 vCPUs around 3.4 GHz (Intel Ice Lake or equivalent), 8 GB RAM, Ubuntu 22.04+ / GLIBC 2.35+, at least 80 GB SSD with growth headroom**. Archives need separate sizing. No new hardware minimum is introduced.

Maintainers use a reproducible dedicated host safe for that baseline, recording CPU/frequency/virtualization, RAM/disk/OS, compiler/executor, commit/features, input ranges and repetitions. Ref-time picoseconds and proof bytes are separate; three-second slots have one-second block ref-time budget. Reservations/test exits are not measured baseline execution. Do not tune live-validator CPUs for benchmarks.

| Path | Scope / accounting |
| --- | --- |
| Native registration | Five proofs, effective-controller conversion, invalid proof, repeated full rotation/purge. Retains upstream base set-keys plus **2,500,000,000 ps**; local measured registration maximum below is covered, with baseline qualification still required. |
| Source activation/restart | Independent actual active/queued **1000/1000**, uniqueness, keys, history/MMR commitments; conservative half-Operational-maximum validation reservation plus real wrapper admission. |
| Full bridge unpause | Full **256/256** verification, Normal; upstream unpause plus structural allowance below and a separate **100,000,000,000 ps / 131,072-byte** validation reserve. Normal admission is covered by the runtime budget check. |
| Enqueue/rejection | Bound/legacy last-free-slot, max payload, structural checks and oversize rejection through 1000. Base enqueue plus **500,000,000 ps, 7 MiB proof bytes and 16 DB reads**. Proof reserve covers full declared authority-vector encoding, not only their length prefixes. |
| Migration | All registered owners, active/queued state, ownership/exposure tries; half-block trie headroom **plus size-dependent DB charges**. Neither 256 nor 1000 caps registered-owner migration input. |
| Session/history/MMR | Exposure pages, retention/pruning, immutable activated snapshots, handover and queue/leaf work. Direct V0 trie hashing avoids a single large ABI staging buffer without changing commitments. Measure full transition block; session rotation already charges maximum block weight, not two reservations. |

### Benchmark calibration

**Local production-profile evidence is not baseline-host qualification.** The 2026-10-08 M4 Max run used macOS 27.2, 14 cores, 36 GiB RAM, Rust 1.99.0-nightly (`87e5904f5`, pinned nightly-2026-07-21), compiled WASM, 50 steps / 20 repeats and max analysis. Full 256/256 unpause recorded 2,000 samples: maximum 12,998,000 ns and 117,598 measured proof bytes. Its separate 100 ms reserve exceeds that observed CPU maximum by 7.69×; measured proof bytes do not replace declared-storage proof bounds. All runtime benchmark correctness cases and the filtered 41-case runtime suite passed; full calibration and supported-state evidence are recorded separately as completed.

Unpause raw JSON SHA-256: `5584fb9d00a093f2a1d8117a3e2565aab0facb4c786adbfd75016c06bb28cff5`; benchmark node SHA-256: `d3480e8d91430bbb9041d3e87e8938533bbc57a85a7ada596d17327e70ccc150`; embedded benchmark WASM SHA-256: `b30e645cde148dc6d824952d5298c42c233d1b3aebab6fe6913f47389e254a03`. This measurement predates only the pricing-reserve update; it is not a deployable artifact or release approval. Retain raw data/logs and qualify the unchanged published baseline separately.

The complete 50-step/20-repeat native calibration retained **14,140 timing samples**. Raw `native-all.json` SHA-256: `f73f3af02a9d7419770fbe35ac52e478a9938888d46b27d8d64c553b6eb1ba1c`. All measured execution/verification cases finished; the original command then hit an upstream per-storage proof-analysis panic (`analysis.rs:286`, empty slopes). CPU max analysis completed successfully from the unchanged raw JSON. JSON-input analysis lacks storage metadata and emits zero estimated proof sizes: **do not install those generated weights**. Keep declared-storage proof bounds and the existing conservative adapters.

| Native path | Samples | Maximum time (ms) | Maximum measured proof (bytes) |
| --- | ---: | ---: | ---: |
| `register_keys` | 20 | 0.155 | 967 |
| `rotate_legacy_keys` | 20 | 0.152 | 1432 |
| `rotate_keys` | 20 | 0.150 | 1432 |
| `reject_registration` | 20 | 0.116 | 0 |
| `purge_keys` | 20 | 0.032 | 1228 |
| `purge_legacy_keys` | 20 | 0.022 | 1228 |
| `activate` | 2000 | 39.358 | 452630 |
| `bridge_unpause_bound` | 2000 | 10.036 | 117598 |
| `bridge_send_bound` | 2000 | 0.347 | 84526 |
| `bridge_send_legacy` | 20 | 0.218 | 66001 |
| `bridge_reject_capacity` | 2000 | 0.261 | 67091 |
| `historical_root` | 2000 | 27.775 | 8702231 |
| `session_start` | 2000 | 19.527 | 8752109 |
| `migrate_session_keys` | 2000 | 686.668 | 73379655 |

The registration allowance alone exceeds its observed maximum by 16.1×; the unchanged source reservation exceeds the 39.358 ms activation maximum. The 0.5 ms structural allowance alone exceeds the complete bound-send and rejection maxima; existing base and DB charges remain additional. The migration figures are not a promise that 10,000 owners fit a block: size-dependent DB charges must still pass supported-state upgrade qualification. History/session measurements cover their named hooks, not a full transition-block baseline qualification.

Ordinary bridge calibration and storage-aware max analysis passed separately. Raw `bridge-all.json` SHA-256: `e6b3a48776131c216c3c2dbb4044d68d276a917752062279c371b8b329bff6a3`; each path has 20 samples. Maximum times: pause 0.007 ms, legacy unpause 0.007 ms, fee update 0.005 ms, binding 0.013 ms, send 0.214 ms, full-queue finalization 0.557 ms. These local measurements do not replace reference-hardware weights.

On the qualified host use production profile, **50 steps / 20 repeats**, durable raw JSON/logs:

~~~sh
: "${BENCH_EVIDENCE_DIR:?Set a durable existing evidence directory}"
cargo build -p gear-cli --profile production --locked \
  --features runtime-benchmarks,runtime-benchmarks-checkers
# This SDK CLI requires output files to exist before generation.
touch "$BENCH_EVIDENCE_DIR/native-cpu.rs" "$BENCH_EVIDENCE_DIR/pallet_gear_eth_bridge.rs"
target/production/gear benchmark pallet --chain=dev --steps=50 --repeat=20 \
  --heap-pages=16384 --pallet=beefy_benchmarks --extrinsic='*' \
  --output-analysis=max --output-pov-analysis=max \
  --json-file="$BENCH_EVIDENCE_DIR/native-all.json" > "$BENCH_EVIDENCE_DIR/native-all.log" 2>&1
# Raw-input CPU analysis avoids the pinned SDK per-storage proof regression bug.
target/production/gear benchmark pallet --json-input="$BENCH_EVIDENCE_DIR/native-all.json" \
  --output-analysis=max --output-pov-analysis=max \
  --output="$BENCH_EVIDENCE_DIR/native-cpu.rs" > "$BENCH_EVIDENCE_DIR/native-analysis.log" 2>&1
target/production/gear benchmark pallet --chain=dev --steps=50 --repeat=20 \
  --heap-pages=16384 --pallet=pallet_gear_eth_bridge --extrinsic='*' \
  --output-analysis=max --output-pov-analysis=max \
  --json-file="$BENCH_EVIDENCE_DIR/bridge-all.json" \
  --output="$BENCH_EVIDENCE_DIR/pallet_gear_eth_bridge.rs" > "$BENCH_EVIDENCE_DIR/bridge-all.log" 2>&1
~~~

These artifacts are nondeployable; `dev` selects benchmark genesis, not fast timing. `activate` independently samples **1..1000 active/queued**; full bound unpause/send **1..256/256**; `bridge_reject_capacity` covers current, next, both and desired oversize through **1000 with full encoded authority values**. Old 256-only source samples are insufficient. Preserve maximum-input observations, rejection state/fee invariants and measured-versus-charged dimensions/margins. FRAME excludes fixture generation/signing/assertions from timing; storage-root work is reported separately.

Measure registration/purge/migration/history/session/MMR and ordinary bridge paths too; extend owner/exposure fixtures for fresh inventory and measure whole session/era hooks. Individual results do not replace exact deployable-WASM staging. Old reference-machine links and Xeon/M4 diagnostics are historical context, not changed hardware requirements or current qualification.

## Evidence status and handover

Older custom-registration qualification results, helper artifacts and local/Hoodi success counts are superseded for this native contract. Compatible full predecessor snapshots remain reusable inputs: pin their original block/hash/root and rerun the changed runtime, rather than downloading again solely because the state is older.

### Supported-state and native restart evidence

The changed mainnet companion passed try-runtime 0.10.1 `--checks all` with spec checks, full decoding, configured try-state checks, idempotence and weight warnings enabled. Only unsupported multi-block simulation was disabled. Reused input: `vara/11000`, finalized block **36,741,398**, hash `0xa83455d6fcdf6c72f1cedad6117ae86dedd8e9716c1755c4f26bd0f13b9256d8`, snapshot SHA-256 `d7089929dc7dfa2dfb6c05395708c9c95a2c52c076cdab2b76d6fe4e18cbe25a`. Its loaded root exactly matched the recorded header root `0xa80e804affa535f70d1364a0f73b38b4a6174221bd33c25ba05e3119183f5a0e`.

The `vara/20100` companion SHA-256 is `53f25d30357f1f4547e4b13875c2efd8c7b28ac5985fa122d66ab382032ba4f6`. Reapplication retained root `0x33b1b600a3ae43347c7a3b9474f2b55a2761faf54b76fb31ff1c821dc75b2e0a`. Reported migration ref-time was **0.66585 s of 1 s**, compressed PoV **157.0 KiB**; the CLI reported no weight safety issues. Staking warnings about nominator stake exceeding bonded stake remained visible. This proves the pinned supported-state rehearsal, not deployable-artifact staging or future state-size capacity.

The disposable native CI fixture passed source activation before binding, domain-authenticated MMR proofs, protected Bob restart without bootstrap Bob, later all-five-key rotation and purge. Before rotation, a fresh **2-of-2 commitment at block 73** exceeded the restart baseline best block 72 (finalized 67, prior commitment 65). BABE/GRANDPA/BEEFY ownership proofs, old-key/purged-owner rejection and wrong-set rejection passed. Public before/after JSON SHA-256: `b1484327f995f0e4fa1bc2246378b3b00fa39dd3cfa94d3c7e712f80565b6b2b` / `b668b3250a6bd9ff69ee1724f37819c040c9c1c8c684a98649e2eff47fdb63f5`. Fast timing is confined to this disposable smoke.

Normal-timing production mainnet and testnet builds passed the runtime import and network-identity checks. Retained compressed WASM SHA-256: mainnet `7b5315797348d25c64f45e3ada4be833dfa9ed26c04fa6bc44658b224602e6e3`, testnet `6ba9b8310ecc5c19119af1878ec7e06e07a1d170121fe79f2afdc08cb18b7863`; matching metadata SHA-256: `8d80e0b960df461b8336bfee4ece964deaeddecef26224301ca87199a36c851c` / `313833576171c1fcd2e8f4b4a17d18324e9c2facd821a61d886b037494c7dcd1`. These are local qualification artifacts, not an approved release. The complete debug all-target/all-feature workspace suite passed **2,268 tests, 19 skipped**; the SDK fixture stays dependency-only so its optional logging-disable feature cannot alter unrelated tests.

Final-source production and try-runtime rebuilds reproduced these WASM hashes; the reused mainnet snapshot passed again with the same root and accounting. Metadata identities were decoded and checked separately: mainnet uses `vara_runtime_prod.scale` (`vara/20100`), testnet uses `vara_runtime.scale` (`vara-testnet/20100`). Do not pair a mainnet WASM with the testnet metadata filename.

Strict all-target/all-feature workspace clippy passed with both the CI-disabled and restored workspace-hack configurations. Generated dependencies were restored and `cargo hakari manage-deps --dry-run` was clean; the all-feature graph does not enable `sp-api/disable-logging`. The explicit SDK ownership/RPC package suite passed **122 tests, none skipped**.

The complete release all-target/all-feature suite passed **2,274 tests, 18 skipped**, using the existing authenticated GitHub environment for the template-list integration request. The initial unauthenticated run passed 2,273 tests and failed that external request; its failure evidence is retained. No test was disabled or weakened.

Local raw evidence is retained under `/tmp/vara-beefy-final-qualified/`; copy it into the reviewed durable release archive before approval. Shared-runtime migration qualification is complete using the retained mainnet snapshot and the separate `dev` tests/build checks. The unavailable public-testnet endpoints do not block this implementation acceptance. Baseline-host measurements, network-specific exact deployable-WASM staging, reviewed release manifests and independent network/destination approvals remain release gates.

Mainnet handover requires immutable reviewed pins/checksums, mainnet metadata/manifest and state inventory, shared migration/proof-retention evidence, baseline measurements, normal-timing native restart/session evidence, finalized approvals/receipts, post-binding bootstrap/accepted roots, destination replay/custody reconciliation and durable relay recovery. Unpublished release facts, exact-artifact staging, baseline calibration and independent destination approvals remain explicit deployment prerequisites, not a requirement to deploy public testnet first. See [migration gates](beefy-migration.md) and [node operation](vara/node/README.md#beefy-upgrade-and-later-activation).
