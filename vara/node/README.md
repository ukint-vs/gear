# Gear Node

Gear Substrate-based node, ready for hacking.

Gear node is a key element of the Vara blockchain network. In a nutshell, it is a standard Substrate node with many low-level modules being used out-of-the-box, specifically, the consensus layer, libp2p networking etc. There are some modifications though, which cater to the specific needs of the Gear runtime as a platform for Wasm-based dApps. The most notable one is a custom block authorship logic brought to ensure that the main invariants the Gear protocol relies on, are upheld:
- the messages queue is processed last in a block and the processing has enough time to run;
- there is always a block within a slot, regardless of potentially indeterministic behavior of the programs execution.

## Building from source

### 1. Install dependencies

#### Ubuntu/Debian
```
sudo apt update
# May prompt for location information
sudo apt install -y git clang curl libssl-dev llvm libudev-dev cmake protobuf-compiler
```

#### MacOS
```
# Install Homebrew if necessary https://brew.sh/
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install.sh)"

# Make sure Homebrew is up-to-date, install openssl
brew update
brew install openssl
```

Additionally, if you use Apple Silicon (M1/M1 Pro/M1 Max), install Rosetta:
```
/usr/sbin/softwareupdate --install-rosetta --agree-to-license
```

#### Windows

Windows 10 is supported with WSL!

- Install WSL and upgrade it to version 2 use instructions from https://docs.microsoft.com/en-us/windows/wsl/install-win10.
- Ensure VM feature is enabled in bios in processor advanced menu.
- Install Ubuntu 20.04 LTS https://www.microsoft.com/store/apps/9n6svws3rx71.
- Launch installed app and setup root user - exit ubuntu app (first time launch takes time).
- Install windows terminal from app store or use VSCode with remote plugin (auto suggested once wsl is detected by VSCode).
- Follow instructions for linux.

### 2. Rust and all toolchains

If Rust is not yet installed, read the [Installation](https://doc.rust-lang.org/book/ch01-01-installation.html) part from [The Book](https://doc.rust-lang.org/book/index.html) to install it.

Make sure the `wasm` target is enabled:
```bash
rustup target add wasm32v1-none
```

Set the environment variables:
```
source ~/.cargo/env
```

### 3. Build the node

Run the following commands to build the node:
```bash
make node-release
```

The resulting binary will be located at `./target/release/gear`.

## Running a dev node

To run a local dev network, execute the following command:

  ```bash
  gear --dev
  ```

By providing an additional argument one can specify the location of the chain database:
  
  ```bash
  gear --dev --base-path /tmp/vara
  ```

Now the dev node is listening on the [default] rpc port 9944: https://polkadot.js.org/apps/?rpc=ws%3A%2F%2F127.0.0.1%3A9944

The list of available subcommands and command-line options can be obtained by running:

  ```bash
  gear --help

  Usage: gear [OPTIONS]
        gear <COMMAND>

  Commands:
    key            Key management cli utilities
    build-spec     Build a chain specification
    check-block    Validate blocks
    export-blocks  Export blocks
    export-state   Export the state of a given block into a chain spec
    import-blocks  Import blocks
    purge-chain    Remove the whole chain
    revert         Revert the chain to a previous state
    try-runtime    Try-runtime has migrated to a standalone CLI (<https://github.com/paritytech/try-runtime-cli>). The subcommand exists as a stub and deprecation notice. It will be removed entirely some time after January 2024
    chain-info     Db meta columns information
    help           Print this message or the help of the given subcommand(s)

  Options:
    ...
  ```
For instance, complete clean-up of a chain state and blockstore can be done by purging the chain:

  ```bash
  gear purge-chain --dev
  ```

## More advanced modes

### Multi-node local Vara network

Running a local testnet with two validator nodes - Alice and Bob, allows to watch the multi-node consensus algorithm in action.
Note that if you launch both nodes on the same machine, you need to specify different ports for each node.

Start the `alice` node first:

  ```bash
  gear --alice --chain=local --base-path ./tmp/alice --port 30333 --rpc-port 9944 --validator
  ```

While the node is starting, inspect the start up log and look for the line that would look like the one below:

  ```bash
  2024-01-01 11:23:05 🏷  Local node identity is: 12D3KooWMar4rG4kfoCZA1sqaY8FqtPDgpBPfDnQ7Md9x6Sdkgw5
  ```
Take note of the node identity string.

Now open another terminal window and start the `bob` node. Note that since both nodes are going to be running on the same machine we should choose different tcp and ws ports (for libp2p and rpc connections) for each node.
Also, we need to specify the `--bootnodes` parameter by providing the multiaddress of the `alice` node to let `bob` know where to look for its peer:

  ```bash
  gear \
    --bob \
    --chain=local \
    --base-path ./tmp/bob \
    --port 30334 \
    --rpc-port 9945 \
    --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWMar4rG4kfoCZA1sqaY8FqtPDgpBPfDnQ7Md9x6Sdkgw5
    --validator
  ```

Having done this, you should see the `bob` node connecting to the `alice` node and starting to produce blocks.

Check the network status at https://polkadot.js.org/apps/?rpc=ws%3A%2F%2F127.0.0.1%3A9944.

### Connect to the Vara mainnet

Running a node that would sync with the Vara mainnet is as simple as running the following command:

  ```bash
  gear --chain=vara
  ```

As before, supplying a variety of CLI arguments allows to customize your node in terms of the chain database location, rpc port, and so on.

### Running an archive node

In some projects it can be useful to store all historical data. To run an archive node, use the following command:

  ```bash
  gear --chain=vara --blocks-pruning=archive --state-pruning=512
  ```
where the `--state-pruning` value specifies the history depth (in terms of the number of blocks) of the state to be kept in the database. All other CLI options apply, as usual.

Turning on the archiving option will significantly increase the disk space usage as well as impact the node's performance. This should be done judiciously.

### Connect to Vara testnet

Finally, calling simply
  
  ```bash
  gear
  ```
will connect you to the default chain, which is Vara testnet.

### Connect to a custom chain

To connect to a custom chain, the first thing one needs to do is to obtain the chain specification JSON file. Then calling the following command will start a node which will then try to connect to the bootnodes from the provided chain specification and start syncing blocks:

  ```bash
  gear --chain=/path/to/your/chain/spec.json
  ```

### Run the node as validator

To run a Vara network validator, start the node with `--validator` and complete the bonding/session registration described in the [Vara validator guide](https://wiki.vara.network/docs/vara-network/staking/validate). For the five-key BEEFY upgrade, use the migration procedure below instead of the wiki's legacy empty-proof registration flow.

### BEEFY upgrade and later activation

For an existing public testnet or mainnet, follow the [ordered operator runbook](../../beefy-deployment.md#existing-testnet-and-mainnet-operator-runbook). It separates release publication, pre-upgrade node/indexing preparation, post-upgrade key registration, governance activation and Ethereum traffic cutover. Install the published release; validators do not need to build the runtime or run maintainer benchmarks.

Deploy compatible nodes and enable offchain indexing before the runtime upgrade.
Use one runtime upgrade, then the standard governance activation call once ready.
This follows the separation used by [Polkadot's BEEFY runtime introduction](https://github.com/polkadot-fellows/runtimes/pull/65)
and later activation on [Kusama](https://kusama.subsquare.io/referenda/343) and
[Polkadot](https://polkadot.subsquare.io/referenda/615). No activation-only Wasm upgrade is needed.

The runtime uses `ext_crypto_ecdsa_verify_prehashed_version_1` for BEEFY key proofs and `ext_trie_blake2_256_root_version_2` for historical ownership roots. Both are supplied by the pinned SDK's `SubstrateHostFunctions` in the node executor. Release qualification checks the built WASM against the explicit host-import allowlist; do not disable that check.

The runtime upgrade leaves BEEFY inactive. The session-key migration supports the
Vara 11000 predecessor, preserves the four existing keys and queued order, and
adds placeholder BEEFY keys. Validators must replace those placeholders with real
node keys before activation; they do not need to do so before the runtime upgrade.

Missing BEEFY authority lists are initialized empty, and the current set gets
a session mapping if absent. Existing BEEFY records and the activation block
remain unchanged.

Root governance first calls `GearEthBridge.bind_destination(chain_id, queue)`.
The chain ID is a nonzero 32-byte **big-endian** Ethereum chain ID and the queue is
the approved original nonzero 20-byte destination contract. This one-time call
uses the actual nonzero source genesis hash from `System.BlockHash(0)`, and derives
`Keccak256("vara/gear-eth-bridge-domain/v2" || sourceGenesis[32] || chainIdBE[32] || queue[20])`.
It rejects rebinding, a pre-existing conflicting domain, and binding after BEEFY
activation. Never use raw `System.set_storage` to initialize a lane. Root remains
trusted to control arbitrary storage, as elsewhere in the runtime.

Every `Session.set_keys` registration now needs a 65-byte ECDSA ownership proof.
The prehash is Keccak256 of SCALE encoding of the tuple
`(b"vara/beefy-session-keys/v1", source_genesis, signed_account, full_session_keys)`;
the domain is a fixed byte array (no compact-length prefix). The signature is
compact `r[32] || s[32] || recovery_id[1]`. It binds the actual signed account,
chain, and all five session keys. Query `Staking.Bonded(stash)` for the actual signer:
legacy controllers and current self-controlled stashes need not use the same account.
The actual signer also needs sufficient **unbonded liquid balance** for transaction
fees; a fully bonded genesis stash cannot pay them from its staking lock.
Registration ownership records are indexed by the converted validator stash; purge
removes that record. Deterministic migration placeholders and invalid SEC1 keys
cannot be registered, even before activation.

The standard `author_rotateKeys` RPC cannot produce this prehash signature from
a keystore-only BEEFY key. Generate and securely retain a separate 32-byte ECDSA
secret, without putting it in command-line arguments, logs, or environment variables.
Install the committed dependency tree before handling secrets (Node 22.19.0,
npm 10.9.3), then run the offline helper with the real source genesis, actual
bonded signer, and current 161-byte encoded `Session.NextKeys` bundle:

```bash
rtk proxy npm ci --prefix scripts --ignore-scripts --no-audit --no-fund
rtk proxy node scripts/beefy-session-proof.mjs \
  --genesis 0xPUBLIC_SOURCE_GENESIS --controller PUBLIC_BONDED_SIGNER \
  --session-keys 0xPUBLIC_ENCODED_SESSION_KEYS < /protected/ecdsa-seed
gear key insert --chain /path/to/chain.json --base-path /path/to/node \
  --key-type beef --scheme ecdsa --suri /protected/ecdsa-seed
```

The protected file contains one 32-byte hex secret prefixed `0x`,
and must be readable only by the operator (for example mode `0600`). The key-insert
command reads the **file contents**, not its pathname as the secret. Alternatively
omit `--suri` to use the CLI's secret prompt. The helper performs no networking,
changes only the appended BEEFY public key, self-verifies the signature, and emits
only public `sessionKeys` and `proof`. Submit those with `Session.set_keys` from
the actual bonded signer. For later full-key rotations, pass the newly generated
bundle to the helper and insert the matching externally generated BEEFY secret;
keep the four other corresponding node keys in the keystore.

If importing through `author_insertKey`, preserve the literal SURI string: a typed
`Text` wrapper can turn a `0x` seed into invalid UTF-8/BIP39 input. Key-presence
queries alone do not prove that the node can load and sign with the private key.
Confirm native quorum signatures in an isolated rehearsal before live activation.

Wait until both the exact active session snapshot and queued session bundle contain
operational proven keys. **Every registered `Session.NextKeys` owner**, including
standby validators, must have a proven valid BEEFY key, or purge its stale registration;
checking only current staking candidates is insufficient. Genesis/migration count
all existing unproved registrations in `Session.PendingRegistrations`; a valid
first rotation or purge decrements it, and activation requires zero. Repeated
proved registrations/purges do not decrement twice. Later permissionless standby
registrations always require a valid proof and do not change the counter: there
is **no global standby-owner admission ceiling** and no activation-time scan of
all modern standby keys. Activation validates at most 256 actual current/queued
authorities and requires the configured `Staking.ValidatorCount` target to be
at most 256. Governance must maintain that committee policy; the SDK consensus
storage bound remains unchanged.

Governance then calls `Beefy.set_new_genesis(delay_in_blocks > 0)`, directly or
through a Root governance wrapper. A configured SDK origin validates the immutable
lane, all current/queued/registered keys, uniqueness, nonempty authority lists,
current session mapping, and exact current/next MMR authority commitments. Invalid
readiness also fails through `dispatch_bypass_filter`; a call filter alone is not
the protection. Activation must target a checked, strictly future block. Deliberate
BEEFY restarts remain supported through the same readiness checks and do not change
the lane, source genesis, MMR, bridge queue, nonce, custody, or public-chain genesis.

BEEFY-only session-key changes preserve the original bridge queue. Actual GRANDPA
authority changes retain the existing delayed queue rollover. Do not reset bridge
storage, the MMR, custody or public-chain genesis as part of BEEFY activation.

While a session clear is pending, shared enqueue rejects all senders, including
governance, with `BridgeCleanupRequired`. The builtin reports that the queue needs
cleanup. Rejected sends preserve the queue and nonce; retry after the delayed clear.
Overflow reset rejects with `InvalidQueueReset` after an append in the current block.
Wait for root finalization and a GRANDPA proof covering the latest overflow block.
Governance retains its pause and capacity exemptions outside the pending-clear window.

The bridge pallet continues to own the original queue, message nonce and history.
The runtime commits its snapshot using leaf-extra version 2:
`2 || "vara" || bridgeDomain[32] || parentTimestampLE[8] || initialized[1] || queueIdLE[8] || root[32]`.
The Keccak hash of these 86 bytes is the MMR leaf extra. Message payloads and
their nonce-based hashing remain unchanged.

MMR insertion begins with the runtime upgrade, before BEEFY activation. Proof-serving
nodes need offchain indexing before that first insertion. Relayers must distinguish
the MMR start block from the later BEEFY genesis and follow GRANDPA authority-set
events even when the bridge queue does not roll over. Ethereum verifier cutover,
custody and replay preservation belong to the existing bridge contracts and actors.
Do not change a live lane binding to introduce another destination.

**Before any real-fund activation**, establish an explicit old/new Ethereum queue
and token-relay freeze/cutoff, and preserve or verifiably hand over the Ethereum
consumed-nonce/replay state and custody. The destination binding changes the
checkpoint domain only: `EthMessageExt::hash` remains the legacy message hash over
nonce/source/destination/payload. A freshly deployed queue with empty replay state
can otherwise execute an already-consumed legacy message attested again under the
new checkpoint domain. Immutable source binding does **not** solve this replay
handover or prove that the destination contract is deployed/ready. Do not clear
or reset the source queue or message nonce. Retain evidence of the external
contract/state cutover and relay cutoff before approving a real-fund activation.

Use `release` for local builds and `production` through the normal release pipeline.
Pin the published node/runtime checksums; use a same-revision normal-timing
try-runtime companion for migration APIs, and exercise the exact deployable WASM
on staging. Neither byte identity between profiles nor try-runtime APIs in the
deployable WASM are assumed.
Mainnet uses the published `production_vara_runtime_v*.wasm` (`vara`, no dev); public testnet
uses `testnet_vara_runtime_v*.wasm` (`vara-testnet`, dev but no fast-runtime).
Both have normal session timing. The release node intentionally embeds the testnet
runtime; this does not change the on-chain runtime of an existing mainnet node.
Select the published artifact and metadata for the actual network, never by profile alone.

The [published validator baseline](https://wiki.vara.network/docs/vara-network/staking/validate#hardware-requirements)
is 2 vCPUs around 3.4 GHz (Ice Lake or equivalent), 8 GB RAM and at least 80 GB SSD
with growth headroom. Indexed archives need separate storage sizing. BEEFY does not
introduce a new hardware minimum here. Maintainers still need to measure migration,
session-history, registration, activation and MMR/bridge work against the charged
allowances on hardware qualified against that baseline. This is execution-budget
qualification, not a task for every validator. See the runbook for the exact paths,
existing benchmark coverage and acceptance evidence.

### Local BEEFY activation rehearsal

Build `cargo build --release -p gear-cli --features fast-runtime` for a **fresh,
disposable** local chain only. This explicit feature implies `dev` and uses
four-slot epochs; the normal epoch remains two hours. The node rejects startup
for chain IDs other than `vara_dev` and `vara_local_testnet`. Do not publish this
Wasm or reuse it against existing chain state. Save production artifacts and their
hashes before a fast build overwrites `target/release/wbuild`, and rebuild the
normal feature set for production qualification afterward.

Start both Alice/Bob validator nodes as above with separate disposable base paths,
adding `--unsafe-force-node-key-generation --force-authoring`,
`--enable-offchain-indexing true`, and `--rpc-methods unsafe`. Keep RPC bound to
loopback. Indexing must be enabled on both nodes from genesis. Then run:

```bash
rtk proxy npm ci --prefix scripts --ignore-scripts --no-audit --no-fund
rtk proxy npm test --prefix scripts
rtk proxy node scripts/beefy-activation-smoke.mjs \
  ws://127.0.0.1:9944 ws://127.0.0.1:9945
```

The test rejects nonlocal RPC/chain identities, uses only valueless development
keys, binds a test destination, registers account-scoped proofs, waits for actual
current/queued changes, and activates through Sudo. It cryptographically checks
real native BEEFY quorum commitments against historical MMR roots before and after
ECDSA rotation, exercises BABE/GRANDPA/BEEFY exported ownership APIs and wrong-set
rejection, and purges the final validator in the current ordering to check all
three authority sets and ownership proofs. JSON output records actual session
indices, commitments, and domain. This is a **source-chain activation rehearsal**,
not a deployed Ethereum bridge/verifier or reference-hardware production test.
Stop both disposable nodes after the rehearsal.

The pinned-tool rehearsal passed with activation at block 35, eight verified
native quorum commitments, two-key rotation, and a two-to-one committee purge.
Both new-key generations became current and queued; all three ownership APIs
returned valid-owner proofs and rejected the purged owner, and GRANDPA/BEEFY
rejected wrong set IDs. The privileged activation call is Operational so its
conservative validation allowance also passes transaction-pool admission.
Evidence: `/tmp/gear-beefy-final-tools-evidence.json`, SHA-256
`cf204a95b579e45e97578def3e6c88684347e7d105ffbb36543da9404cc5dbd5`.
Both isolated fixture nodes were stopped afterward.

For this single-block-only runtime, use try-runtime with `--disable-mbm-checks`
and retain `--checks all`. Its multi-block simulation fabricates the predecessor
version; do not weaken the migration guard to accept that synthetic state.
