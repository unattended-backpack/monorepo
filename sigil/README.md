# Sigil

This Rust project builds the `sigil` binary. This is the client of the Sigil rollup.

## Building

To assist in easy management of forked crates, as well as the automatic generation of Docker images for robust testing, this client must be built using the provided `build` script instead of the typical `cargo build`.

To run the build script, one must specify a `.env.build` file containing an `OVERRIDES` environment variable tracking any local forks of dependencies. This variable should contain one line for each forked dependency. The format of each line in `OVERRIDES` is a `;`-delimited tuple of the format `crate_name;local_path;git_remote;EXCLUSIONS` where `EXCLUSIONS` is a `,`-delimited list of excluded directories under the top-level directory of the `local_path`. An example `.env.build` file might look like this:

```
OVERRIDES="
libp2p;../../rust-libp2p/libp2p/;https://github.com/unattended-backpack/rust-libp2p.git;target,scripts
"
```

This means that, for the `libp2p` crate, we should build using the contents of the local `../../rust-libp2p/libp2p/` directory if it exists. If it does not exist, we should use the remote fork of `libp2p` hosted at [`https://github.com/unattended-backpack/rust-libp2p.git`](https://github.com/unattended-backpack/rust-libp2p). When the build script generates a Docker image of our project, the `target` and `scripts` folders will be excluded.

It is our hope that we have successfully encapsulated this admittedly-convoluted build process to make the developer experience of contributing to Sigil as smooth as possible.

## Testing

Once built, the project may be tested as usual using the standard `cargo test`. Some integration tests rely on the ability to access a Docker image of the client to test inter-client communications.  

* If the tests are manually cancelled, make sure to also manually stop any docker containers started by the test.

## Running

Sigil requires a `.toml` config file to run.  By default, sigil looks for a `sigil.toml` in the same directory, but this can be changed by setting env var `CONFIG_TOML_PATH=<path to your .toml config file>`.  For an example: make a copy of `example_sigil.toml` and rename it: `$ cp example_sigil.toml sigil.toml`.  If connecting to an existing p2p network, the multiaddr and peer_id of at least 1 public node in the network must be supplied.

Once built (see [Building](#building)), the binary can be run using the generated docker image `$ docker run sigil:dev` or cargo `$ cargo run`.
