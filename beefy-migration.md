# Migrating the bridge to BEEFY

This is the migration roadmap and release-gate checklist for replacing the Gear-to-Ethereum root-proving path with native BEEFY commitments and MMR proofs. It is not authorization to upgrade a live chain or move bridge assets. For commands, deployment addresses and current evidence locations, use [BEEFY deployment](beefy-deployment.md).

**2026-09-22 update:** a separate local Gear → Hoodi message-only deployment now exists, using real BEEFY verification, an immutable adapter and MessageQueue. The runner supports retained-message restart, real destination delays/finality and an optional handover follower. The dated publication inventory and planning baseline below are retained; claims that no Hoodi script/command exists are superseded for this demonstration only. Production transaction recovery, existing-chain migration, token bridging and governance integration remain release gates. No commit, push or PR update was performed during this deployment.

**Current real-network procedure:** use the [testnet/mainnet operator runbook](beefy-deployment.md#existing-testnet-and-mainnet-operator-runbook). Source migration and key/activation guards are implemented and qualified by the evidence below; the older local-v1 planning inventory is historical. Production artifacts are built/published by the normal release pipeline, not by each validator. Node updates and indexing precede the runtime upgrade; real five-key registration follows it; destination binding precedes BEEFY activation; Ethereum traffic cutover requires its own approval.

## Historical publication inventory (2026-09-21)

Remote branches and PR state were checked on **2026-09-21**. Recheck before release; branch names are not release pins.

| Repository | Pushed branch | Remote head | Pull request | State at inspection |
| --- | --- | --- | --- | --- |
| `gear-tech/gear` | `beefy-mmr-phase-0-1` | `0b13f2c61b0e5d9844c7efd12727487a2fdb8c63` | [#5642](https://github.com/gear-tech/gear/pull/5642), based on `master` | Open, draft |
| `gear-tech/gear` | `beefy-bridge-e2e` | `ab72e2134968251e91f400120718491622a36bae` | [#5644](https://github.com/gear-tech/gear/pull/5644), based on `beefy-mmr-phase-0-1` | Open, draft |
| `gear-tech/gear-bridges` | `beefy-local-e2e` | `14b44ccfa729ce67e961e8e3698fc84455fdc5f5` | [#860](https://github.com/gear-tech/gear-bridges/pull/860), based on `main` | Open, draft |

Gear [#5643](https://github.com/gear-tech/gear/pull/5643) is a closed duplicate, not another dependency. Both Gear BEEFY branches also existed on the `ukint-vs/gear` fork at the same heads. PR #5642 tracks `ukint-vs/gear:beefy-mmr-phase-0-1`; PR #5644 tracks `gear-tech/gear:beefy-bridge-e2e`. Pushing only the upstream foundation branch does not update #5642. Keep the fork head and the upstream branch used as #5644's base coordinated when publishing the stack.

**The secure-v1 changes and these guides are local, uncommitted work on the two E2E branches at inspection time. They are not contained in the remote heads above.** A fresh clone of those heads does not reproduce the completed secure-v1 implementation. No commit, push, PR edit or merge was performed while preparing these guides.

The separate Bridge branch `security/high-risk-fixes` is pushed at `2f9994ea99e47820ee39326e82612fd19216d345`. No PR with that head was returned in `gear-tech/gear-bridges`. Its existence is not evidence that its changes are integrated into the BEEFY branch or that the deferred MessageQueue recovery work is complete.

Read-only status refresh:

~~~sh
rtk gh pr view 5642 --repo gear-tech/gear --json state,isDraft,headRefOid,baseRefName,url
rtk gh pr view 5644 --repo gear-tech/gear --json state,isDraft,headRefOid,baseRefName,url
rtk gh pr view 860 --repo gear-tech/gear-bridges --json state,isDraft,headRefOid,baseRefName,url
rtk git -C "$GEAR_DIR" ls-remote origin 'refs/heads/*beefy*'
rtk git -C "$BRIDGE_DIR" ls-remote origin 'refs/heads/*beefy*'
~~~

Before publication, review the dirty worktrees, publish the runtime and Bridge v1 changes together as reviewable commits, update the affected PR scope, and record the final commit and artifact hashes. The Gear base must precede its dependent PR. Keep `fast-runtime` opt-in and out of production builds. Do not treat the currently pushed draft stack as a production release.

## What is implemented

The secure-v1 local implementation has passed normal and `dev` runtime suites, Rust relay tests, the full Foundry suite, API artifact comparisons, and a real two-authority rehearsal. The recorded secure-v1 milestone rehearsal used 28.59 seconds for preparation and 108.50 seconds for the separate live sequence. It accepted 40 authenticated updates and delivered two real messages across two key rotations, a natural queue clear and a stale-proof rebuild.

This proves the consensus-to-message path on fresh local chains. It does **not** prove an existing-network storage upgrade, an asset migration, a production relay service or a Hoodi deployment.

The implemented path is:

~~~text
Finalized Gear blocks + native BEEFY signatures
  -> source collector and signature validation
  -> BeefyClient: newest MMR leaf, source identity/time, authority handovers
  -> VaraQueueRootVerifier: historical queue leaf under the live latest anchor
  -> existing MessageQueue: root registration, maturity, nonce replay checks
  -> message destination / token application
~~~

BEEFY replaces the **Gear-to-Ethereum consensus/root proof path**. It does not replace Ethereum-to-Gear checkpoint/light-client programs, Ethereum event verification, token accounting, governance programs or application relayers. Inventory and retain those components until their own cutover is verified.

### Protocol invariants operators must preserve

- The current source snapshot is exactly 86 bytes: version **2**, `vara`, immutable bridge domain, parent timestamp in milliseconds, initialized flag, queue ID and queue root. The domain binds the actual source genesis, destination chain ID and queue address. Native commitments and the version-0, 113-byte outer MMR leaf stay unchanged. The historical v1 Hoodi client is not evidence of compatibility with this v2 snapshot; verify the paired bridge release.
- The v1 queue envelope is exactly `576 + 32N` bytes with at most 256 proof items. V0 proofs are not a compatibility path.
- `A` is the first MMR insertion block. `G` is BEEFY activation. Discover both; do not assume they are equal after a live upgrade.
- Bootstrap uses a verified finalized checkpoint `C`, with `0 < A < C <= u32::MAX`, the parent timestamp at `C-1`, and the exact ordered current/next authority tuples. Genesis and timestamp are nonzero; next ID is current ID + 1.
- The newly deployed client starts with an accepted MMR root of zero. A later signed update must be accepted before any queue proof can register a root.
- Every update authenticates the newest leaf, including same-set updates. Source time cannot decrease, may be at most 120 seconds ahead, and expires strictly after 24 hours. Receipt time does not renew trust. An expired client cannot self-revive.
- Historical queue timestamps are not independently expired. Their proofs need the latest nonzero anchor of a live client. Queue clears therefore do not erase already authenticated history.
- Authority sets are limited to 1..256. Native claimed quorum is `N - floor((N-1)/3)`. The selected signature count is capped at `floor(N/3)+1`, with fixed floors 86/86. These are distinct checks.
- Interactive RANDAO delay/window are fixed at 128/24 blocks. Do not weaken constants to make a deployment or test pass.
- The adapter is immutable for one client, MessageQueue address and destination chain ID. Direct calls from an operator are not substitutes for registration through that queue.

## Gate 1: make the source-chain upgrade safe

**Source implementation completed and snapshot-qualified; network rollout and measured capacity qualification remain operational gates.** The following checklist describes required acceptance evidence, not six unimplemented source features. Follow the current operator runbook for responsibility and ordering.

The runtime adds a fifth BEEFY key to `SessionKeys`. Existing four-key SCALE storage cannot simply be decoded as the new type. The warning next to `SessionKeys` in the Gear runtime explicitly requires a session-key migration in the same upgrade.

Required work:

1. Implement and review the existing-state migration, including current/queued session keys and ownership bookkeeping. Define how each real BEEFY key is obtained. A zero or placeholder key is not an operational validator key.
2. Coordinate key registration and activation with validators. Verify key ownership, uniqueness and the exact authority ordering used to compute Ethereum roots.
3. Exercise the upgrade against a representative existing-chain snapshot, with pre/post storage checks, preserved BABE/GRANDPA operation and multiple actual session transitions.
4. Qualify production CPU/storage/proof work, including historical trie construction. Conservative block-weight reservations and native diagnostic timings are not calibrated production benchmarks.
5. Build without `fast-runtime` and verify the normal production timing, authority limits, runtime versioning and upgrade authorization.
6. Bring up independent indexed proof archives **before the runtime upgrade / first MMR insertion**, not merely before activation. Verify genesis, `A`, `G`, finalized MMR counts and historical proof availability.

**Upgrade exit evidence:** migration/proof-retention results, node/operator preparedness, indexed archives, weight/capacity qualification and published release hashes. After the runtime upgrade, collect actual five-key registration, active/queued propagation, native BEEFY signatures and handovers before bridge enablement. Requiring live BEEFY signatures before its introducing runtime upgrade would be circular.

### Existing-mainnet input checkpoint (2026-10-07)

Read-only acquisition used finalized Vara block **36,741,398**, hash `0xa83455d6fcdf6c72f1cedad6117ae86dedd8e9716c1755c4f26bd0f13b9256d8`, through `https://rpc.vara.network` / `wss://rpc.vara.network`. Runtime metadata at that exact hash identifies **vara/11000**, state version 1. This is genuine existing state, not a synthetic genesis fixture; no transactions or validator secrets were used.

- Current session **15540**; active validators **59**, queued validators **59**, configured `Staking.ValidatorCount` **59**, distinct stored `Session.NextKeys` registrations **112**, and original ownership records **448**. All four BABE/GRANDPA/ImOnline/AuthorityDiscovery arrays match the queued order, with all 59 owners aligned in each array.
- The required-state snapshot includes complete Session, Historical, Staking (36,801 keys), BABE, GRANDPA, ImOnline, AuthorityDiscovery, BEEFY, GearEthBridge, Balances, Treasury and GearScheduler prefixes. Real storage at the same hash supplements System number/runtime-upgrade information, Treasury/builtin accounts and existing pallet storage versions. It contains **87** historical roots: 85 prior sessions, the current session and the queued future session.
- The scoped input is not a whole-chain snapshot. Separate whole-chain `--checks all` qualification subsequently passed below; neither snapshot test alone proves live consensus liveness.
- Activation requires every one of the **112 initial legacy registrations** to obtain verified proof-of-possession registration or be legitimately purged, including inactive owners. The migration counter tracks these original obligations; new proven standby registrations do not create an arbitrary global standby cap. Destination domain is unset and BEEFY must remain disabled until the approved binding and real active/queued BEEFY keys are ready.
- The initial desired committee is already 59, so this checkpoint needs no economic target change to satisfy the bridge limit. Governance must keep future elected committees within the current bridge verifier's **256**-authority limit; the source BEEFY configuration remains **100,000**, not silently truncated to 256.

Input SHA-256 pins: required-pallet snapshot `28a7ed21c9cb1f75ad11c0cf3905b8311429ab8e29d67a338943fe177f070587`; supplemented predecessor snapshot `6715790e4609fcee2aae00248a1066eccccddc9f4a21bccb300fb0af5712b376`; real supplemental storage `2a327466216e66c507a12f3fd8f3da75abd55da7cfc5ff29a102f700cab42389`. See the deployment guide for exact execution commands and artifact qualification scope.

Counter-aware normal-timing **20100** qualification first passed on this genuine scoped input:

- The complete native `Executive::try_runtime_upgrade` ran every configured migration with pre/post checks, initialized **PendingRegistrations = 112**, preserved all **85** prior-session roots byte-for-byte, and rebuilt the current/future active snapshots.
- One BABE ownership proof was obtained from the actual **11000** runtime using `state_call` at the pinned block (slot 597127833), not reconstructed from a test fixture. It remained valid after migration and an actual native session rotation. All **59 BABE + 59 GRANDPA** new active-authority proofs also verified before/after rotation: **119** retained real-state proofs total. BEEFY remained inactive and the bridge domain remained absent. This is externalities execution, not live network finality.
- Native migration wall/process CPU was **132.938 / 132.588 ms**; session rotation was **2.167 / 2.170 ms**, on an Apple M4 Max. The runtime returns aggregate migration weight **501,825,000,000 ps**, or **50.1825%** of its nominal 1-second block ref-time budget. These measured local durations are not calibrated production weights; the migration reservation is conservative, not a measured historical-trie benchmark.
- CLI 0.10.1 exercised the scoped WASM with `--checks pre-and-post`, default spec/idempotence/storage-decoding/weight checks and no warning suppression: exit 0. It decoded **13,537,711** bytes of loaded storage, retained identical roots on the second upgrade, and reported **149.0 KiB** compressed PoV and **50.18%** declared ref-time usage.

Qualification WASM SHA-256: `4417279b59b766c1ad894db989b6d47141f33e58c2f8f83e3cb11e5e8467b5a0`; native rlib SHA-256: `ab1cd33c027025d57cf313777970cbed2940126162fc7b2b0283ec46c624341a`. Features are normal `default,std,try-runtime`, with neither dev nor fast-runtime; these are **release-profile qualification artifacts, not an approved production build**.

### Whole-chain qualification

The completed genuine snapshot contains **798,801** storage keys and **2,084,893,951** bytes at the same finalized predecessor hash. SHA-256: `d7089929dc7dfa2dfb6c05395708c9c95a2c52c076cdab2b76d6fe4e18cbe25a`. The final normal-timing try-runtime WASM (`329cf47b95a5aa6ffc7da62a078894aeee0f00e9169a20bf659b5dff90cbd018`) passed `--checks all`, including all configured pallet try-state checks, storage decoding, migration idempotence and unsuppressed weight checks.

The unmodified snapshot root reported by remote externalities, `0xa80e804affa535f70d1364a0f73b38b4a6174221bd33c25ba05e3119183f5a0e`, also matches `chain_getHeader` at the pinned finalized hash. The separate successful two-node activation/rotation/purge rehearsal is recorded in `beefy-deployment.md`; it used explicit fast dev timing, not normal mainnet timing.

The second upgrade preserved root `0xdc9c2db9e8afa471bfe7dd39e9f1c98f29d09380f2ec0d1e56ca961551c37843`. Declared migration ref-time was **0.501625 s / 50.16%** of the 1-second budget; compressed PoV was **157.0 KiB**. The log contains staking warnings about historical nominator exposure exceeding current bonded stake, but the all-pallet check exited **0** and reported no weight safety issue. This does not replace reference-hardware calibration or approval of production-profile artifacts.

### Final registry-scaling correction

The new custom WASM benchmarks exposed that the old fixed half-block reservation hid growth in the permissionless legacy registry. Migration now retains half-block trie headroom **and adds** size-dependent upstream database read/write charges. This changes accounting, not migrated keys or activation policy; no standby-owner cap is introduced. A regression at the sampled 10,000-owner boundary fails with the old accounting and passes with the correction.

The final normal-timing try-runtime artifact, SHA-256 **`685ead7f39b71dd1708910e500529097864293ce95a80a628d572f7180e795e6`**, passed the same genuine whole-chain `--checks all` rehearsal, storage decoding and idempotence. Aggregate declared ref-time is **0.66590005 s / 66.59%**, compressed PoV **157.0 KiB**, with no weight safety issue. Its idempotence root is `0x9cf3adc6f3266a15b2ad58d962a506fd50b6e6fb8ff3b5983278f6a325ff765b`; the unmodified input root still matches the pinned chain header. This supersedes the fixed-reservation artifact above.

The full 126-test source suite and 56 native BEEFY client tests pass, as do strict all-target/all-feature Clippy checks for the runtime and affected pallets. Ten custom and six bridge benchmark cases pass native and actual WASM verification. The offline helper's three test groups also pass after extraction from the release bundle, and a native two-validator run exercised the actual helper through activation, rotation and purge. Qualification remains distinct from publication/approval of a production release and from public-network governance execution.

The ownership regressions also pass with BABE debug assertions enabled: fixtures initialize BABE before direct session rotation, matching the runtime hook contract rather than relying on release builds to omit its assertion. Native BEEFY client all-target/all-feature Clippy passes as well.

## Gate 2: preserve queue state and define recovery

**Not completed by the local v1 milestone.**

The current MessageQueue has no public verifier setter. It is UUPS-upgradeable and stores the verifier during initialization. Replacing the proof path on an existing proxy needs a reviewed, governance-authorized upgrade mechanism, not another `initialize` call, a raw storage write or a fictitious `setVerifier` transaction.

Recommended production direction: preserve the existing MessageQueue proxy and its storage when feasible. This keeps processed-message nonces, registered roots, maturity timestamps, governance references and application references in one state history. The authorized verifier-switch/recovery mechanism is still work to implement and review.

Before any switch:

- Fix and test the deferred duplicate-root maturity and conflicting-root lifecycle behavior. V1's adapter does not repair those existing semantics.
- Specify recovery after client expiry or an irrecoverable source mismatch. The client and adapter have immutable bindings and no resetter. A replacement client/adapter requires a separately authorized queue migration.
- Ensure recovery authorization is usable when the old proof client is expired. A recovery procedure that can only arrive through that expired verifier is circular.
- Preserve processed nonces, root availability, original maturity rules and pending deliveries. Test governance/admin messages as well as ordinary messages.
- Distinguish queue pause, challenge-root and emergency-stop behavior. A paused application is not proof that all root submissions or privileged operations have stopped.

A **fresh queue** is appropriate for an isolated test deployment. In production it starts with an empty replay ledger. Pointing the same assets/applications at both old and new queues can permit duplicate execution of historical messages. A fresh-queue production migration therefore needs an explicit source cutoff, replay-state strategy, asset/application authorization migration and reconciled pending-message ledger. Do not run both queues as unrestricted authorities for the same assets.

**Exit evidence:** storage-layout checks, upgrade tests from deployed state, duplicate/conflicting-root regressions, expiry recovery, governance reachability and replay/maturity preservation.

## Gate 3: deploy durable relaying and application integration

The dated local-v1 milestone did not complete production relaying. The later bounded Hoodi command/follower is recorded above; it does not establish durable public-network recovery or token integration.

A public-testnet/production service must add:

- External Gear/Ethereum endpoints, explicit chain identity and contract bindings, secure fee-payer handling and durable state directories.
- Persistent accepted checkpoints, source cursors and transaction records; restart reconciliation against on-chain client/queue state rather than blind replay.
- Complete finalized commitment catch-up, mandatory handover retention, subscription reconnection and unavailable-history failures that remain visible.
- Ethereum transaction replacement/reconciliation, receipt and finality policy, nonce ownership and stale-anchor proof regeneration. Give each independent submitting process its own fee-payer account.
- Root and message-proof capture before the source queue clears; enough retained state to finish deliveries after restarts and clears. Reconcile inclusion reorgs and discard noncanonical proofs. The bounded local runner currently aborts if a message finalizes in a different block from its initially retained proof; this occurred during documentation validation.
- Source/destination clock checks and freshness alerts well before the 24-hour boundary. Never synthesize a source timestamp to match Ethereum time.

Integrate this service with the existing token-message delivery path. Retain Ethereum-to-Gear services, configure the intended Ethereum network in its light-client/event programs, and update contract/program addresses, token mappings, indexers and UI configuration as needed. Verify real token accounting in both directions; the local mock receiver is not a token-system acceptance test.

**Exit evidence:** restart, catch-up, archive-loss and network-outage tests; source handovers during downtime; no duplicate execution; reconciled token balances; documented service configuration and monitoring.

## Gate 4: qualify on isolated networks

Follow the [deployment guide](beefy-deployment.md) in this order:

1. Fresh local Gear + Anvil using the existing verified runner.
2. Persistent isolated Gear + Hoodi using the implemented bounded message-only demonstration. Use valueless assets; public testnet/mainnet acceptance additionally needs private operational keys and the reviewed v2-compatible destination/relay release.
3. A sustained rehearsal using normal source timing, realistic validator sets and an existing-state upgrade snapshot. Include outages, restarts, delayed transactions, source/destination finality and expiry recovery.
4. Independent review of the modified consensus verifier, adapter, runtime migration, queue upgrade/recovery and operator procedures.

The recorded Foundry submission measurements below are per-call test observations, not a total transaction budget or a production forecast. Interactive values cover `submitFinal`, not the preceding initial submission and RANDAO transaction.

| Validators | Selected signatures | Fiat-Shamir submission gas | Interactive final gas |
| --- | --- | --- | --- |
| 59 | 20 | 375,330 | 372,144 |
| 150 | 51 | 905,614 | 897,445 |
| 256 | 86 | 1,472,624 | 1,473,756 |

Budget deployment, calldata, initial/RANDAO calls, queue registration and message delivery separately on the target network.

## Gate 5: execute the approved cutover

This is an operator sequence to complete only after Gates 1-4 pass. Exact governance calldata and new service commands must come from the reviewed release, not from this roadmap.

1. **Freeze the release.** Record Gear and Bridge commit IDs, runtime/node hashes, contract artifacts, chain identities, addresses, policy constants and the approved governance actions. Verify public PR CI/review status against those exact commits.
2. **Inventory pending work.** Select a finalized source cutoff and reconcile pending roots, messages, processed nonces, escrow, supplies and application balances. Archive proofs/state needed to finish the old path.
3. **Upgrade and observe the source.** Follow the operator runbook: update nodes/indexing first; enact the runtime migration; register proven keys; wait for active/queued propagation; approve and bind the destination; schedule BEEFY through governance; verify native finality and handovers. Approve the destination queue address before immutable binding, even when contract deployment must await a later authenticated bootstrap.
4. **Authenticate bootstrap.** Compare the same finalized `C > max(A, G)` and hash on two nodes, validate the newest native proof and parent timestamp, and record source genesis, immutable domain, `A`, `G` and complete authority tuples. Reacquire a checkpoint if it is no longer fresh.
5. **Deploy and verify contracts.** Use the reviewed existing-queue upgrade path or approved fresh-queue migration at the address already bound on the source. The adapter must match that exact chain/queue and v2 domain. For a new queue use the reviewed deterministic deployment plan, never ad hoc nonce offsets. Verify code, governance, bindings and initial zero-root client state.
6. **Shadow the new consensus relay.** Continue strictly after bootstrap `C`, including intervening handovers. Require a later authenticated nonzero root and exact accepted source time. Compare observations before authorizing application traffic.
7. **Switch and drain deliberately.** Execute the authorized proof-path switch, stop the old root writer at the defined cutoff, register a canary root and verify matured delivery/replay rejection. Complete the reconciled old pending ledger; do not send legacy ZK proofs to the v1 adapter.
8. **Enable applications.** Switch the intended delivery/indexing/UI configuration and verify actual token transfers and balances in both directions. Keep reverse-direction infrastructure running.
9. **Retire only the replaced components.** Stop old Gear-to-Ethereum proof-generation workers after the drain is complete. Keep recovery archives and any proving/verification components still used elsewhere.

### Abort and recovery rules

Abort before opening traffic on source disagreement, missing archives, wrong bytecode/bindings, invalid authority sets, expired trust, skipped handovers or an unreconciled pending ledger. Preserve evidence and stop the affected writers.

Before any irreversible user execution, an approved plan may allow returning to the old writer. After user messages execute, recovery is a state-reconciliation operation: do not roll back to stale databases or a queue with an older replay ledger. Do not wipe source node data, reset client time or redeploy a queue as an emergency shortcut.

## Handover package

A migration is complete only when the release bundle contains:

- Reviewed/merged PRs and immutable source/build pins for both repositories.
- Source-chain upgrade and validator-registration evidence.
- Deployment receipts, verified bytecode, governance ownership and immutable bindings.
- Authenticated bootstrap and subsequent accepted commitments, with source times and proofs.
- Pending-message/asset reconciliation and successful delivery/replay evidence.
- Durable relay deployment/configuration, ownership of fee-payer accounts, monitoring and tested restore/recovery instructions.
- Operator sign-off for disabling the old Gear-to-Ethereum proof path.

Implementation references in `gear-tech/gear-bridges`: `ethereum/src/beefy/BeefyClient.sol`, `ethereum/src/VaraQueueRootVerifier.sol`, `ethereum/src/MessageQueue.sol`, `ethereum/test/Base.sol` and `tools/beefy-relay/src/{source.rs,rehearsal.rs}`. Resolve them against the reviewed Bridge release revision. The source-runtime counterparts are `vara/runtime/vara/src/{lib.rs,bridge_leaf.rs,migrations.rs}` in this Gear repository.
