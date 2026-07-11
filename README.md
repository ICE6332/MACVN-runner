# VNRT

VNRT is an experimental Win32-compatible runtime for selected 32-bit visual novels. It loads PE32 executables, interprets x86 guest code, and maps the Win32 APIs that a target actually reaches.

This is not Wine and is not intended to run arbitrary Windows software.

## Current status

- PE32 loading, imports, TLS, TEB/PEB, memory, files, and callbacks work.
- The runner covers a growing Kernel32/User32/GDI32/WinMM surface and can capture Guest DIB presentation as RGBA frames.
- A backend-neutral graphics layer now owns real wgpu textures; wgpu selects Metal on macOS while leaving Vulkan, D3D12, and GLES available on other hosts.
- The primary Chinese `euphoriaCN.exe` target now unpacks its child, loads the modeled D3D9/DirectSound modules, discovers `pac`, and opens the real YPF archives.
- The current frontier is CPU throughput while the engine indexes/decrypts those archives; no Guest D3D method has executed yet.

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
- `crates/vnrt-gfx`: backend-neutral GPU resources
- `crates/vnrt-gfx-wgpu`: Metal/Vulkan/D3D12/GLES backend through wgpu
- `crates/vnrt-x86`: x86 interpreter
- `crates/vnrt-*32`: target-driven Win32 API surfaces
- `docs/NEXT_STEPS.md`: roadmap
- `docs/INTERPRETER_OPTIMIZATION.md`: performance design and unsafe policy
- `docs/TARGET_EUPHORIA.md`: HD comparison-target notes
- `docs/TARGET_EUPHORIA_CN.md`: primary Chinese target notes

Licensed under MIT or Apache-2.0.
