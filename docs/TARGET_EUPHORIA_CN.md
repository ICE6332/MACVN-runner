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
- The observed NTDLL export census now also covers the loader's registry,
  process, thread, file, object, and virtual-memory probes. The latest resolved
  frontier is `NtDuplicateObject`; unresolved names remain explicit resolution
  stubs so a real call fails at the exact API boundary.

The first target-relevant NTDLL semantics are implemented:

- `NtAllocateVirtualMemory` for the current process.
- `NtReadVirtualMemory` and `NtWriteVirtualMemory` for current-process Guest memory.
- `RtlInitUnicodeString` using the 32-bit `UNICODE_STRING` layout.
- `RtlAcquirePebLock` and `RtlReleasePebLock` as documented single-thread no-ops.

The next step is to finish the export census, then implement the first executed
section/view or object API from its actual arguments. The HD executable remains
useful as a comparison path but is not the primary target.
