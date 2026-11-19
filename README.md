# repro-check

A small tool for checking whether Rust builds are reproducible, and for finding why they are not.

`repro-check` copies a crate into two directories with different absolute paths and builds each one with `cargo build --release`, with `SOURCE_DATE_EPOCH` fixed. It then compares every final artifact byte by byte. For each artifact that differs, it reports:

- the size and the number of differing bytes
- the first differing offsets
- the printable strings found in only one build, classified as build path, hash/ID or other

## Usage

```sh
cargo build --release
./target/release/repro-check <crate-dir> [--remap] [--brepro] [--keep]
```

| Flag | Effect |
|---|---|
| `--remap` | adds `RUSTFLAGS=--remap-path-prefix=<build dir>=/build` (the standard Rust fix) |
| `--brepro` | adds `-C link-arg=/Brepro` (MSVC linker: deterministic timestamp and debug signature) |
| `--keep` | keeps the temporary build directories for inspection |

To test builds with full debug info, set `CARGO_PROFILE_RELEASE_DEBUG=2`.

## First results (rustc 1.99.1, 5 crates)

The test crates were `hello`, plus small binaries using `anyhow`, `regex`, `serde_json` and `clap` (in `subjects/`).

**Linux** (Ubuntu 22.04, x86_64-unknown-linux-gnu)

| Configuration | Result |
|---|---|
| release | identical in 5/5 |
| release + full debug info | differs in 5/5: the absolute build path is embedded |
| debug info + `--remap-path-prefix` | identical in 5/5 |

**Windows** (x86_64-pc-windows-msvc)

| Configuration | `.exe` | `.pdb` |
|---|---|---|
| release | differs in 5/5 (~20 bytes) | differs in 5/5 |
| `--remap-path-prefix` | differs in 5/5 | differs in 5/5 |
| `/Brepro` | identical in 5/5 | differs in 5/5 |
| remap + `/Brepro` | identical in 5/5 | differs in 5/5 |

On Windows the standard Rust fix is not enough. The `.exe` differences come from the MSVC linker's timestamp and debug signature, which `/Brepro` removes. The `.pdb` files stay non-reproducible because the linker records absolute object and `.rlib` paths, which rustc cannot remap.

See [FINDINGS.md](FINDINGS.md) for details and next steps.

## Next

- Scale to 20-30 popular crates, including ones with `build.rs`, proc-macros and C dependencies.
- Localize the build step responsible for each difference using system-call tracing (`strace` on Linux), following the RepTrace approach (Ren et al., ASE 2019).
- Evaluate fixes for `.pdb` reproducibility on Windows.
