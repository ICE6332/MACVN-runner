# Target: euphoria Chinese launcher

The Chinese executable is the primary compatibility target. Commercial files remain local under the ignored `tests/targets/` directory.

## Local fingerprint

- Image: `tests/targets/euphoria/inspect/euphoriaCN.exe`
- SHA-256: `ea36a153a20453fc9ff4254233050528320c5f83a6a06c63c3de052b42940072`
- Format: PE32 GUI, Intel 80386
- Preferred base: `0x00400000`
- Entry point: `0x00402798`
- Image size: `0x00744000`
- Static imports: 49, all from Kernel32

## Observed path

The launcher initializes its CRT and Japanese/Chinese ANSI conversion tables, opens its own executable, reads the large overlay, and unpacks executable code into the Guest heap. The release Runner needs roughly 200 million interpreted instructions to cross this unpacking phase.

The unpacked loader uses `INT3` as its unresolved-export failure path. Runtime now dispatches breakpoint exceptions through the real 32-bit Guest SEH chain rather than treating them as unsupported instructions. Stack diagnostics identified the missing exports in order.

Completed loader surface:

- `LoadLibraryA/W` for modeled Host DLLs.
- Automatically generated synthetic module images for every registered API module.
- A loadable synthetic `ntdll.dll`.
- Observed NTDLL exports: `NtContinue`, `NtQuerySection`, `RtlNtStatusToDosError`, `RtlDosPathNameToNtPathName_U`, `RtlFreeUnicodeString`, `NtCreateSection`, and `NtMapViewOfSection`.

The NTDLL names are currently resolution stubs. The next step is to continue until the loader invokes one, then implement the section/view semantics from the actual arguments. The HD executable remains useful as a comparison path but is not the primary target.
