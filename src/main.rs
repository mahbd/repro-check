//! repro-check: build a Rust crate twice from different directories and report
//! which output files differ and which embedded strings cause the difference.
//!
//! Usage:
//!   repro-check <crate-dir> [--remap] [--keep]
//!
//! --remap  also pass RUSTFLAGS=--remap-path-prefix=<build dir>=/build to test the usual fix
//! --keep   keep the temporary build directories

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

fn copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "target" || name == ".git" {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        if entry.file_type()?.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Build the crate in `dir` with its own target directory. Returns the release output dir.
fn build(dir: &Path, remap: bool, brepro: bool) -> io::Result<PathBuf> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("--release").arg("--quiet").current_dir(dir);
    cmd.env("CARGO_TARGET_DIR", dir.join("target"));
    cmd.env("SOURCE_DATE_EPOCH", "1700000000");
    let mut flags = Vec::new();
    if remap {
        flags.push(format!("--remap-path-prefix={}=/build", dir.display()));
    }
    if brepro {
        // MSVC linker: deterministic PE timestamp / debug-directory signature
        flags.push("-C link-arg=/Brepro".to_string());
    }
    if flags.is_empty() {
        cmd.env_remove("RUSTFLAGS");
    } else {
        cmd.env("RUSTFLAGS", flags.join(" "));
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("cargo build failed in {}", dir.display()),
        ));
    }
    Ok(dir.join("target").join("release"))
}

/// Final artifacts only (top level of target/release): executables, libraries, debug info.
fn artifacts(out: &Path) -> io::Result<Vec<String>> {
    let mut v = Vec::new();
    for e in fs::read_dir(out)? {
        let e = e?;
        if !e.file_type()?.is_file() {
            continue;
        }
        let n = e.file_name().to_string_lossy().to_string();
        let keep = [".exe", ".dll", ".rlib", ".pdb", ".so", ".dylib", ".a", ".lib"]
            .iter()
            .any(|s| n.ends_with(s))
            || !n.contains('.');
        if keep {
            v.push(n);
        }
    }
    v.sort();
    Ok(v)
}

/// Printable ASCII strings of length >= min.
fn strings(data: &[u8], min: usize) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let mut cur = Vec::new();
    for &b in data {
        if (0x20..0x7f).contains(&b) {
            cur.push(b);
        } else {
            if cur.len() >= min {
                set.insert(String::from_utf8_lossy(&cur).to_string());
            }
            cur.clear();
        }
    }
    if cur.len() >= min {
        set.insert(String::from_utf8_lossy(&cur).to_string());
    }
    set
}

fn classify(s: &str, a: &Path, b: &Path) -> &'static str {
    let lower = s.to_lowercase();
    let hit = |p: &Path| {
        let p = p.display().to_string().to_lowercase();
        lower.contains(&p) || lower.contains(&p.replace('\\', "/")) || lower.contains(&p.replace('\\', "\\\\"))
    };
    if hit(a) || hit(b) || lower.contains("build_dir_number_two") || lower.contains("repro-check\\a\\") || lower.contains("repro-check/a/") {
        "build-path"
    } else if s.chars().filter(|c| c.is_ascii_hexdigit()).count() >= 16 && !s.contains(' ') {
        "hash/id"
    } else {
        "other"
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: repro-check <crate-dir> [--remap] [--keep]");
        std::process::exit(2);
    }
    let src = fs::canonicalize(&args[1])?;
    let remap = args.iter().any(|a| a == "--remap");
    let keep = args.iter().any(|a| a == "--keep");
    let brepro = args.iter().any(|a| a == "--brepro");
    let name = src.file_name().unwrap().to_string_lossy().to_string();

    // per-run folder so concurrent runs never delete each other's builds
    let base = env::temp_dir().join(format!("repro-check-{}", std::process::id()));
    let dir_a = base.join("a").join(&name);
    let dir_b = base.join("build_dir_number_two").join(&name);
    let _ = fs::remove_dir_all(&dir_a);
    let _ = fs::remove_dir_all(&dir_b);
    copy_dir(&src, &dir_a)?;
    copy_dir(&src, &dir_b)?;

    println!("crate: {name}  remap: {remap}  brepro: {brepro}");
    let out_a = build(&dir_a, remap, brepro)?;
    let out_b = build(&dir_b, remap, brepro)?;

    let files = artifacts(&out_a)?;
    let mut differing = 0;
    for f in &files {
        let (da, db) = match (fs::read(out_a.join(f)), fs::read(out_b.join(f))) {
            (Ok(x), Ok(y)) => (x, y),
            _ => {
                println!("  {f}: missing in one build");
                continue;
            }
        };
        if da == db {
            println!("  {f}: IDENTICAL ({} bytes)", da.len());
            continue;
        }
        differing += 1;
        let diff_bytes = da.iter().zip(db.iter()).filter(|(x, y)| x != y).count()
            + da.len().abs_diff(db.len());
        println!(
            "  {f}: DIFFERENT (sizes {} vs {}, ~{} differing bytes)",
            da.len(),
            db.len(),
            diff_bytes
        );
        let offsets: Vec<String> = da
            .iter()
            .zip(db.iter())
            .enumerate()
            .filter(|(_, (x, y))| x != y)
            .take(24)
            .map(|(i, _)| format!("0x{i:x}"))
            .collect();
        println!("      first differing offsets: {}", offsets.join(" "));
        let sa = strings(&da, 8);
        let sb = strings(&db, 8);
        let only_a: Vec<&String> = sa.difference(&sb).collect();
        let mut kinds = BTreeMap::new();
        for s in &only_a {
            *kinds.entry(classify(s, &dir_a, &dir_b)).or_insert(0) += 1;
        }
        println!("      strings only in build A: {} {:?}", only_a.len(), kinds);
        for s in only_a.iter().take(4) {
            let short: String = s.chars().take(110).collect();
            println!("        [{}] {}", classify(s, &dir_a, &dir_b), short);
        }
    }
    println!(
        "RESULT {name}: {} of {} artifacts differ{}{}",
        differing,
        files.len(),
        if remap { " +remap" } else { "" },
        if brepro { " +brepro" } else { "" }
    );
    if !keep {
        let _ = fs::remove_dir_all(&base);
    }
    Ok(())
}
