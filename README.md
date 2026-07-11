# VNRT

VNRT is an experimental Win32-compatible runtime for selected 32-bit visual novels. It loads PE32 executables, interprets x86 guest code, and maps the Win32 APIs that a target actually reaches.

This is not Wine and is not intended to run arbitrary Windows software.

## Current status

- PE32 loading, imports, TLS, TEB/PEB, memory, files, and callbacks work.
- The headless runner covers a growing Kernel32/User32/GDI32/WinMM surface.
- The current euphoria HD trace reaches its license dialog and exits cleanly.
- The next milestone is the first native SDL3 window; local target progress is currently blocked by a missing `system/YSCom/YSCom.exe`.

Game files are not included. Use only files you are legally allowed to run.

## Run

Requires Rust 1.97 or newer.

```bash
cargo test --workspace
cargo run -p vnrt-inspect -- path/to/game.exe
cargo run -p vnrt-runner -- path/to/game.exe --max-instructions 1000000
```

Enable the optional SDL3 dependency with:

```bash
cargo check --workspace --all-features
```

## Layout

- `apps/vnrt-runner`: command-line runner
- `apps/vnrt-inspect`: PE32 metadata inspector
- `crates/vnrt-runtime`: loader/runtime composition
- `crates/vnrt-x86`: x86 interpreter
- `crates/vnrt-*32`: target-driven Win32 API surfaces
- `docs/NEXT_STEPS.md`: roadmap
- `docs/TARGET_EUPHORIA.md`: current target notes

Licensed under MIT or Apache-2.0.
