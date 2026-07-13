# VNRT

VNRT is an experimental Win32-compatible runtime for selected 32-bit visual novels. It loads PE32 executables, interprets x86 guest code, and maps the Win32 APIs that a target actually reaches.

This is not Wine and is not intended to run arbitrary Windows software.

## Current status

- PE32 loading, imports, TLS, TEB/PEB, memory, files, and callbacks work.
- The runner covers a growing Kernel32/User32/GDI32/WinMM surface and can capture Guest DIB presentation as RGBA frames.
- A backend-neutral graphics layer now owns real wgpu textures; wgpu selects Metal on macOS while leaving Vulkan, D3D12, and GLES available on other hosts.
- A bounded Host media layer normalizes PNG/JPEG/BMP/GIF/WebP/TGA/TIFF/ICO/DDS to RGBA8 and WAV/OGG/MP3/FLAC/AAC/M4A/AIFF to interleaved f32 PCM.
- The primary Chinese `euphoriaCN.exe` target now unpacks its child, opens the real YPF archives, creates its modeled main window, and enters real resource lookup.
- Read-only archives stream from Host files, so the 1.6 GB `cg.ypf` is no longer copied into RAM.
- Cooperative Guest threads cover `CreateThread`/`ResumeThread`, blocking `WaitFor*`, and `SetEvent` wakeups for the post-resource worker kickoff without Host-native threads.
- Post-wait path now survives window show and the first GDI/logprint probe; the former null execute was a 52-byte `logprint!Test` ABI cleanup mismatch. The next frontier is the longer post-probe path toward first presentation.

Game files are not included. Use only files you are legally allowed to run.

## Run

Requires Rust 1.97 or newer.

```bash
cargo test --workspace
cargo run -p vnrt-inspect -- path/to/game.exe
cargo run -p vnrt-inspect -- path/to/game.exe --census
cargo run -p vnrt-runner -- path/to/game.exe --max-instructions 1000000
cargo run --profile frontier -p vnrt-runner -- path/to/game.exe \
  --max-instructions 10000000000 --dump-first-frame first-frame.png
```

For repeated deep-target runs, use the optimized fast-link profile:

```bash
cargo run --profile frontier -p vnrt-runner -- path/to/game.exe --max-instructions 10000000000
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
- `crates/vnrt-media`: common image/audio decoding and texture upload
- `crates/vnrt-x86`: x86 interpreter
- `crates/vnrt-*32`: target-driven Win32 API surfaces
- `docs/NEXT_STEPS.md`: roadmap
- `docs/INTERPRETER_OPTIMIZATION.md`: performance design and unsafe policy
- `docs/TARGET_EUPHORIA.md`: HD comparison-target notes
- `docs/TARGET_EUPHORIA.zh-CN.md`: HD comparison-target notes (Chinese)
- `docs/TARGET_EUPHORIA_CN.md`: primary Chinese target notes
- `docs/TARGET_EUPHORIA_CN.zh-CN.md`: primary Chinese target notes (Chinese)

Licensed under MIT or Apache-2.0.
