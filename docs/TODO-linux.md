# TODO: Linux & Raspberry Pi support

**Status: deferred.** macOS is the primary tester platform for now. This document records
what is *already in place* (the hooks) and what remains, so the build-out can happen later
without re-discovery.

The goal: a Linux tester — including a Raspberry Pi (ARM, often headless) — can install,
connect, and run the familiar with the same ease as a Mac tester.

## Hooks already in place (done — keep these)

These are deliberately cross-platform already, so Linux is a fill-in, not a rewrite:

- **`author_tool` prompt** (`crates/cycle/src/lib.rs`) — branches by `std::env::consts::OS`:
  on Linux it tells the model to use `/proc`, `free`, `df`, `ip addr`, `nproc`, `vcgencmd`
  and to avoid macOS-only `sysctl`/`vm_stat`/`top -l 1`. This is what makes the run-code
  feature behave on a Pi.
- **`exec`** (`crates/exec/src/lib.rs`) — runs scripts via POSIX `sh -c` with `ulimit -t` +
  a wall-clock timeout. Already portable.
- **`sense`** (`crates/sense/src/lib.rs`) — already has Linux fallbacks for memory
  (`/proc/meminfo`) and network interfaces.
- **LLM adapter** (`llm/call_llm.sh`) — POSIX `sh` + `python3`; runs on Linux/Pi as-is
  (python3 is usually present).
- **CI** — `.github/workflows/ci.yml` builds + clippies on `ubuntu-latest`, so the workspace
  already compiles clean on Linux.

## Remaining work (in priority order)

1. **Headless Pi has no GUI → a CLI `familiar connect` command.**
   egui needs X11/Wayland, so a headless Pi can't use the Glass's Connect wizard. Mirror its
   logic terminal-side: prompt for a key (OpenRouter is enough), install the adapter, write a
   0600 `key.env`, and open `allow_llm` in `boundary.json`.
   - Reuse target: the wizard's `connect_llm` / `test_llm` in `crates/glass/src/main.rs`, and
     the embedded `ADAPTER_SH` (`include_str!("../../../llm/call_llm.sh")`).
   - Consider extracting the connect logic (adapter install + key.env write + boundary
     open) into a small shared helper so the GUI and CLI share one implementation. Keep the
     boundary-write **out of the kernel** — widening stays a human act, performed by the
     human's instrument (Glass or CLI), never the factory.
   - Add `Some("connect") => cmd_connect(rest)` in `crates/cli/src/main.rs`.

2. **Camera discovery on Linux (v4l2 / libcamera).**
   `crates/vision/src/lib.rs` currently discovers cameras via macOS `system_profiler` under
   `#[cfg(target_os = "macos")]`. Add a Linux path: enumerate `/dev/video*` (v4l2) and/or
   `libcamera`. Only needed if a tester uses the eye; the consent gate (Law III) is unchanged.

3. **Distribution per architecture + a Linux service unit.**
   - Prebuilt binaries for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` (Pi).
   - An `install.sh` + a **systemd** unit for the daemon (Linux's launchd equivalent). The
     macOS `daemon install` path (`crates/cli/src/daemon.rs`) writes a launchd LaunchAgent;
     add a sibling that writes a `~/.config/systemd/user/` unit (or `/etc/systemd/system/`).
   - Document the default data dir for a packaged install (today `./familiar_data`,
     cwd-relative; a service wants an absolute path, e.g. under `~/.local/share/familiar/`).

4. **Nicety: CPU brand on Linux.**
   `crates/sense/src/lib.rs` reads `sysctl machdep.cpu.brand_string` (empty on Linux). Add a
   `/proc/cpuinfo` fallback (`model name`, or `Model` on a Pi) for nicer host facts.

## Notes

- The same familiar binary runs on Mac / Linux / Pi; each install has its own data dir and
  its own tool library, so there is no cross-platform script-reuse hazard.
- `std::env::consts::OS` / `ARCH` are compile-time-correct for the target the binary was
  built for — the right basis for the per-OS branches above.
