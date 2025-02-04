# Sigil

Extracted source and build code for all the binaries that make up Sigil.

At its current state, Sigil is an OP stack rollup that uses `op-succinct-proposer` instead of optimism's proposer.  `op-succinct-proposer` allows the requesting and
posting of ZK proofs of the current chain state onto Ethereum.  You can read
more about the OP stack architecture here: <https://docs.optimism.io/builders/chain-operators/architecture>

# /op-succinct

original repo: <https://github.com/succinctlabs/op-succinct/>
forked version tag: `op-succinct-v1.0.1`

Contains `op-succinct-proposer` and `op-succinct-server` ("proof server"").
`op-succinct-proposer` monitors the L2 chain and periodically sends a request for
a proof to `op-succinct-server` in the form of a block range.  `op-succinct-server`
is a small server that accepts requests for proofs and delegates the actual proof
generation to either a local CUDA accelerated prover (a docker image that is
started by the server) or the sp1 prover network (currently only runs the CUDA
prover).  When a proof is done, it returns the proof to `op-succinct-proposer`
who then posts the proof on-chain.  See `op-succinct/example.env` for required env
vars.

`op-succinct/proposer/succinct/bin/single-block-proof.rs` is a binary used for
debugging isolated proof requests and isn't used in prod.

# /op-geth

original repo: <https://github.com/ethereum-optimism/op-geth>
forked version tag: `v1.101411.5`

Contains the execution node for the op-stack chain.

## jwt.txt

`op-geth` and `op-node` communicate over the engine API authrpc. This communication
is secured using a shared secret. You will need to generate a shared secret and
provide it to both `op-geth` and `op-node` when you start them. In this case, the
secret takes the form of a 32 byte hex string. Run the following command to
generate a random 32 byte hex string:

```
openssl rand -hex 32 > jwt.txt
```

## genesis.json

`op-geth` requires a `genesis.json` configuration file to run.  You can find it
at `op-geth/genesis.json`.

# /optimism

original repo: <https://github.com/ethereum-optimism/optimism/>
forked version tag: `op-node/v1.10.3`

Contains the op-stack chain `op-batcher` and `op-node` ("rollup node").
See `optimism/example.env` for required env vars.
`op-node` can be built with `make op-node` in the `optimism` folder.
`op-batcher` can be built with `make op-batcher` in the `optimism` folder.

## `op-node`

Run with

```
./op-node --l1=$L1_RPC_URL --l2=http://localhost:8551 --rpc.addr=0.0.0.0 --l2.jwt-secret=jwt.txt --l1.beacon=https://prettiest-summer-spring.ethereum-holesky.quiknode.pro --l2.enginekind=geth --sequencer.enabled --sequencer.l1-confs=5 --verifier.l1-confs=4 --rollup.config=./rollup.json --p2p.disable --rpc.enable-admin --p2p.sequencer.key=$GS_SEQUENCER_PRIVATE_KEY
```

### jwt.txt

`op-geth` and `op-node` communicate over the engine API authrpc. This communication
is secured using a shared secret. You will need to generate a shared secret and
provide it to both `op-geth` and `op-node` when you start them. In this case, the
secret takes the form of a 32 byte hex string. Run the following command to
generate a random 32 byte hex string:

```
openssl rand -hex 32 > jwt.txt
```

### rollup.json

`op-node` requires a `rollup.json` configuration file to run.  You can find it at
`optimism/rollup.json`.

## `op-batcher`

```
./op-batcher \
  --l2-eth-rpc=http://localhost:8545 \
  --rollup-rpc=http://localhost:9545 \
  --poll-interval=1s \
  --sub-safety-margin=6 \
  --num-confirmations=1 \
  --safe-abort-nonce-too-low-count=3 \
  --resubmission-timeout=30s \
  --rpc.addr=0.0.0.0 \
  --rpc.port=8548 \
  --rpc.enable-admin \
  --max-channel-duration=25 \
  --l1-eth-rpc=$L1_RPC_URL \
  --private-key=$GS_BATCHER_PRIVATE_KEY
```

# Maintaining

The repos in this folder were dragged in via `git subtree`, a less burdensome
alternative to git submodules.  See below for how to change versions of these
dependencies.  All commands are run from the repository root `monorepo/`.

Example: changing version of `optimism` to the release tag `v99.99.99`.

```bash
git remote add sigil/optimism https://github.com/ethereum-optimism/optimism.git

git fetch sigil/optimism

git subtree merge -P sigil/optimism --squash tags/v99.99.99
```

### IMPORTANT MAINTAINING NOTES

- The `--squash` IS VERY IMPORTANT.  Otherwise it will drag in the entire history
of the optimism repo.

- For git subtree commands the other dependencies are named similarly: prefixed by
`sigil/`.  `op-geth` is `sigil/op-geth` and `op-succinct` is `sigil/op-succinct`

- On the merge commit after doing the `subtree merge`, add the version and
dependency that you changed for easier bookkeeping.  Also make sure to change
the version in `monorepo/sigil/README.md`
