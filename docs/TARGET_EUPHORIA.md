# Target: euphoria HD

This is the first target-driven compatibility track. Commercial executable and
resource files remain local under the ignored `tests/targets/` directory; this
document records only technical metadata and Runtime observations.

## Local target fingerprint

- Main image: `tests/targets/euphoria/inspect/euphoriaHD.exe`
- SHA-256: `c0e3aa244cd145951804a0cde2d9ce4f60b4dc5cabd5f1c720719cf2180aec4a`
- Format: PE32 GUI, Intel 80386
- Preferred image base: `0x00400000`
- Entry point: `0x004652c6`
- Image size: `0x00590000`
- Base relocations: none
- Static imports: 258

Import counts by module:

| Module | Imports |
| --- | ---: |
| KERNEL32.dll | 125 |
| USER32.dll | 76 |
| GDI32.dll | 18 |
| WINMM.dll | 18 |
| SHELL32.dll | 7 |
| ADVAPI32.dll | 4 |
| ole32.dll | 3 |
| VERSION.dll | 3 |
| COMCTL32.dll | 1 |
| d3d9.dll | 1 |
| DSOUND.dll | 1 |
| IMM32.dll | 1 |

The image imports `COMCTL32.dll!#17` and `DSOUND.dll!#1` by ordinal. Runtime
ordinal binding uses stable `dll!#ordinal` Host Call keys and no longer blocks
image loading.

## Dynamic compatibility queue

The local runtime set now contains the main EXE, required root configuration
files and DLLs, and 3.1 GiB of active `pac/*.ypf` archives. The redundant
`cg.ypf_old` backup and nested patch archives remain unextracted.

Observed and passed on the real execution path:

1. Win10 version, CRT startup, private heaps, recursive critical sections, and
   dynamic TLS initialization.
2. CP932/UTF-16 conversion, CTYPE tables, case mapping, process/system info,
   x87 control-word setup, and conservative CPU feature detection.
3. Named mutexes, events, waits, PE resources, AppData/Documents lookup, COM
   apartment initialization, and root file enumeration through real
   `WIN32_FIND_DATAA` records.
4. Display capability probing through User32/GDI32, including optional
   `GetProcAddress` fallbacks, followed by the target's observed MMX table-fill
   sequence.
5. WinMM startup cleanup commands (`stop/close ysmcimovie`) and legacy global
   memory cleanup.

The Runtime now suspends Host Calls, enters stdcall Guest callbacks through a
protected return sentinel, supports nested synchronous callbacks, and resumes
the original Host caller with the requested result. The real dialog procedure
successfully processes `WM_INITDIALOG`, a nested `SendMessageA`,
`WM_COMMAND/0x40A`, and `EndDialog`.

Tracing the real dialog identified it as the engine's license-information
dialog rather than the graphics settings dialog. The original target package
does not contain the required `system/YSCom/YSCom.exe`; the file is absent from
the outer ZIP, both nested patch archives, and every extracted YPF directory.
The engine treats this as a fatal initialization failure, displays the license
dialog, then performs a clean shutdown with exit code 0. Absolute paths under
the configured Guest root and Windows-style case-insensitive path lookup are
now supported, so `euphoriaHD.exe`, `yscfg.dat`, and `yssfs.dat` are found; the
remaining `YSCom.exe` failure is a real target-material gap.

The next target step is to supply the matching `system/YSCom/YSCom.exe` from a
complete installation, rerun the existing trace, grow User32 only from newly
executed calls, and attach the first SDL3 window when `CreateWindowExA` is
reached.

Static import presence does not imply that every API must be implemented. APIs
remain target-driven and are added only when an executed path reaches them.
