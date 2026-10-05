# Gear Node

Gear Substrate-based node, ready for hacking :rocket:

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

To run a Vara network validator, the node has to be started with the `--validator` flag. To become the Vara mainnet validator a few additional steps need to be made which include registering yourself as a candidate (by sending an extirnsic to the network), bonding the necessary minimum amount of Vara tokens and configuring session keys. The process is described in detail in the [Vara Network Wiki](https://wiki.vara.network/docs/staking/validate/).

### BEEFY upgrade and later activation

The runtime upgrade leaves BEEFY inactive. The session-key migration supports the
Vara 11000 predecessor, preserves the four existing keys and queued order, and
adds placeholder BEEFY keys. Validators must replace those placeholders with real
node keys before activation; they do not need to do so before the runtime upgrade.

Before activation, check the current, queued and eligible validators' BEEFY keys
and bind `GearEthBridge.BridgeDomain` to the approved destination chain and original
queue. On an existing chain, governance can use the existing Root
`Utility.batch_all` call to set that storage value through `System.set_storage`
and call `Beefy.set_new_genesis(delay_in_blocks = 1)` atomically. Reject an already
active BEEFY instance or a conflicting lane binding in the activation preflight.
Root remains trusted; this procedure does not add a new key-admission framework.

BEEFY-only session-key changes preserve the original bridge queue. Actual GRANDPA
authority changes retain the existing delayed queue rollover. Do not reset bridge
storage, the MMR, custody or public-chain genesis as part of BEEFY activation.

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

Use `release` for local builds and migration rehearsals. Reserve `production`
for the actual release. Before proposing an upgrade on mainnet, qualify the final
release Wasm against pinned full state and retain runtime-weight/capacity evidence.
