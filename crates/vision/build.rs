//! Compiles the Swift `familiar-eye` camera helper next to the cargo binaries, on macOS,
//! best-effort. It is intentionally non-fatal: off macOS, or when `swiftc` is missing, it
//! emits a warning and moves on, so the Linux CI green bar is unaffected and a host without a
//! Swift toolchain simply has no camera capture (the Rust side fails closed) rather than a
//! broken build. The release `.app` installer compiles the same source independently.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let src = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("eye")
        .join("familiar-eye.swift");
    println!("cargo:rerun-if-changed={}", src.display());

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    // OUT_DIR is .../target/<profile>/build/<pkg>-<hash>/out; the binaries land three
    // ancestors up, in .../target/<profile>. Put familiar-eye there so it's a sibling of
    // `familiar` and resolves the same way at runtime.
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let Some(bin_dir) = out_dir.ancestors().nth(3) else {
        return;
    };
    let dest = bin_dir.join("familiar-eye");

    let swiftc_ok = Command::new("swiftc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !swiftc_ok {
        println!("cargo:warning=swiftc not found — camera capture helper (familiar-eye) not built");
        return;
    }

    let status = Command::new("swiftc")
        .arg("-O")
        .arg(&src)
        .arg("-o")
        .arg(&dest)
        .status();
    match status {
        Ok(s) if s.success() => {}
        _ => println!(
            "cargo:warning=failed to compile familiar-eye.swift — camera capture unavailable"
        ),
    }
}
