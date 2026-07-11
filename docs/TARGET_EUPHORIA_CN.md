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

The export census now completes and the launcher maps an unpacked child image
around `0x70100000`. Its import loader reads the synthetic DLL export images and
has progressed through Kernel32 and NTDLL imports into ordinary Guest code.

The executed compatibility path now includes:

- A modeled `advapi32.dll` with process-token, `TokenUser`, SID string, and
  cross-DLL allocation/free semantics.
- Separate virtual address reservation and page commitment, plus
  `VirtualProtect` permission transitions and readable Host thunk facades.
- Windows full/short/long path conversion, file-time queries, and NT DOS-path
  conversion with owned `UNICODE_STRING` buffers.
- Target-executed x87 integer loads/stores, float loads/stores, stack behavior,
  and multiply/add operations.

The current frontier is a bad indirect Guest control target (`0x00000002`) after
the first x87-heavy main-program path. The next diagnostic should retain recent
indirect `CALL`/`JMP`/`RET` origins so the value can be traced to its producer.
The HD executable remains useful as a comparison path but is not the primary
target.
