# Contributor guide

## Code

### Layout

Code layout, as of 8/2020:

```
.
├── build.rs - script to compile protos, run when you execute `cargo build`
├── etc
│   ├── fix_protos.sh - strips gogoproto annotations from protobufs
│   ├── travis_before_install.sh - run in the `before_install` phase of TravisCI
│   ├── travis_setup.sh - run in the `setup` phase of TravisCI
│   └── travis_test.sh - run in `test` phase of TravisCI
├── examples
│   ├── hello_world.rs - the hello world of pachyderm
│   └── opencv.rs - the canonical opencv demo, ported to rust
├── fuzz - fuzz tests of pachyderm internals
│   └── fuzz_targets
│       ├── extract_restore.rs - fuzzes extract/restore-related functionality in pachyderm
│       └── pfs.rs - fuzzes PFS
├── proto/ - a copy of the protobufs from the pachyderm project
├── rustfmt.toml - config for rustfmt
└── src
    └── lib.rs - the library source code
```

### Style

Code is formatted via rustfmt. [Make sure to install
it](https://github.com/rust-lang/rustfmt#quick-start), then simply run
`cargo fmt`.

### Rebuilding protobuf code

To rebuild protobuf code, run:

```bash
# copy the protobufs into ./proto
PACHYDERM_ROOT="<path to pachyderm>" make clean proto
# re-compile the protos and library
cargo build
```

## Testing

### Examples

Because the library is 100% auto-generated, there aren't any unit tests. You
can run the examples as a poor man's integration tests via:

```bash
cargo run --example hello_world -- "grpc://<pachd hostname>:30650"
cargo run --example opencv -- "grpc://<pachd hostname>:30650"
```

### Fuzzers

Since rust has excellent fuzzer facilities, we have written a couple of
fuzzers of internal Pachyderm functionality that use rust-pachyderm.

#### Extract/restore

This fuzzes extract/restore-related functionality, which is important since
it's employed for backup/restore and migrations in Pachyderm. To run:

```bash
PACHD_ADDRESS="grpc://<pachd hostname>:30650" make fuzz-extract-restore
```

#### PFS

Fuzzes PFS functionality. To run:

```bash
PACHD_ADDRESS="grpc://<pachd hostname>:30650" make fuzz-pfs
```

### Linting

To lint, install [clippy](https://github.com/rust-lang/rust-clippy) and run
`cargo clippy`. This is unlikely to find anything actionable since the library
is nearly 100% auto-generated.

## Documentation

Docs for releases are available on [docs.rs](https://docs.rs/pachyderm). You
can also build the docs yourself locally:

```bash
cargo doc --package pachyderm
```

## Releasing

To make a new release, from the master branch:

* Update `CHANGELOG.md`
* Run `cargo publish`
