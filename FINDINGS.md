# repro-check: first findings (2026-11-19)

Setup: Windows 11, rustc/cargo 1.99.1, MSVC toolchain (x86_64-pc-windows-msvc), `cargo build --release`.
Each crate is copied to two folders with different absolute paths and built separately, with `SOURCE_DATE_EPOCH` fixed.
Subjects: 5 small binaries: hello (no dependencies), and one each using anyhow, regex, serde_json, clap (clap brings 9 dependencies).

| Configuration | .exe | .pdb |
|---|---|---|
| default | differs in 5/5 (16-20 bytes) | differs in 5/5 |
| `--remap-path-prefix` | differs in 5/5 | differs in 5/5 |
| `-C link-arg=/Brepro` | **identical in 5/5** | differs in 5/5 |
| remap + /Brepro | identical in 5/5 | differs in 5/5 |

Observations
1. The .exe differences are only the ~20 bytes the MSVC linker writes for the PE timestamp and debug-directory signature (offsets 0x3204.. in hello.exe). The build path is not embedded in release executables here. The MSVC linker flag `/Brepro` alone makes the .exe bit-for-bit reproducible. `--remap-path-prefix`, the usual Rust advice, does not help on Windows.
2. The .pdb debug files are not reproducible under any configuration tried. They store the linker command line (`/OUT:<abs path>`) and absolute paths of every object file and dependency `.rlib` under `target/release/deps`. Those paths come from the linker, not rustc, so `--remap-path-prefix` cannot remove them.
3. The number of build-path strings in the .pdb grows with the dependency count (hello 6, clap 18).

Linux (Ubuntu 22.04 VM, same rustc 1.99.1, x86_64-unknown-linux-gnu)

| Configuration | binary |
|---|---|
| release (default) | identical in 5/5 |
| release + remap | identical in 5/5 |
| release + full debug info (`CARGO_PROFILE_RELEASE_DEBUG=2`) | differs in 5/5: one embedded build-path string; size differs by exactly the path-length difference, so every later byte shifts |
| debug info + `--remap-path-prefix` | identical in 5/5 |

For clap (which has a proc-macro dependency), the debug-info build also had 43 differing hash/ID strings (symbol hashes). These disappeared with remap too.

Conclusion so far: on Linux the standard Rust fix (`--remap-path-prefix`) is sufficient. On Windows/MSVC it is not. Executables need `/Brepro`, and `.pdb` files remain non-reproducible because the linker records absolute object paths.

Next steps
- Repeat on Linux (Ubuntu VM in VMware) with `diffoscope` for structured diffs, and `strace -f -e trace=file,process cargo build` to localize which build step writes each differing byte (RepTrace-style).
- Scale to 20-30 popular crates, including crates with build.rs, proc-macros and C dependencies (cc crate), where non-determinism is more likely.
- Test the PDB fix candidates: `/PDBALTPATH`, building in a fixed path, `-C split-debuginfo`.
