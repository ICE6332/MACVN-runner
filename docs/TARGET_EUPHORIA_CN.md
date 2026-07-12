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

The invalid `0x00000002` control target was a secondary SEH symptom. Retained
`CALL`/`JMP`/`RET` history exposed the actual missing `SetErrorMode` export, and
the loader now reports unresolved imports through its own `Import Error` dialog
path. A modeled `psapi.dll` supplies `GetModuleBaseNameA/W` for that diagnostic.

The Kernel32 export census has since crossed directory creation, file
attributes, memory status, local time, process status, locale, file-mapping
probes, and the initial thread API family. File-mapping and `CreateThread`
facades currently fail with `ERROR_NOT_SUPPORTED` if they are actually invoked;
full mapping-object lifetime and multi-context Guest scheduling remain explicit
mainline work rather than silent fake success.

The census has continued through fixed Tokyo time-zone/system-time data,
write-range validation, sandboxed file copying, mutable standard handles synced
to process parameters, ANSI environment aliases, locale validation/info, and a
real single-locale Host-to-Guest enumeration callback.

The latest compatibility pass added a complete writable file-handle lifecycle,
including creation dispositions, buffered writes, flush-on-close, and
`SetEndOfFile`. Software `RaiseException` calls now enter the real Guest SEH
chain with exception flags and information records. Environment mutations
rebuild the ANSI/UTF-16 process blocks and update the PEB, while pointer probes,
string helpers, file/directory removal, and system-directory queries cover the
remaining observed Kernel32 imports.

The child has now crossed the Kernel32 import census and is deep into
`user32.dll`. The runtime now keeps real process-local window classes and atoms,
Guest WndProc pointers, window objects, titles, styles, placement, visibility,
enabled state, menus, regions, cursor position, and display mode. `CreateWindowEx`
constructs a Guest `CREATESTRUCT` and dispatches `WM_NCCREATE`/`WM_CREATE` to the
registered WndProc.

The initial thread also has a modeled message queue covering post, peek, get,
translate, dispatch, and quit messages. Default-window processing, invalidation,
`BeginPaint`/`EndPaint`, display enumeration, clipboard handles, icon/menu
lifetime, input state, and the common geometry APIs are in place.

The observed GDI32 pass now has memory and window DCs, selected objects, DIB
sections, bitmap transfer, text/font probes, regions, and frame capture. A DIB
presented to a window is normalized into a top-down RGBA8 `WindowFrame`, ready
for a native backend to upload when the target uses this path.

The child has also crossed Shell32, Advapi32, COMCTL32, Ole32, WinMM, IMM32, and
Version imports. WinMM has real sandboxed MMIO file reads/seeks and RIFF chunk
descent/ascent; unavailable audio devices and input methods remain explicit.
The import census now also crosses D3D9 and DirectSound discovery, followed by
the target's absent optional `logprint.dll` plugin (`Test`, `Test2`, and the same
numbered cdecl probe family).

The transient-code failure near `0x100a817b` was caused by a legacy packer that
requested 16 bytes from the heap and wrote a 34-byte executable trampoline. The
Guest heap now preserves requested-size accounting while placing tiny blocks in
64-byte physical size classes, matching the slack expected from an NT heap.

Two Win32 filesystem details then unlocked the real content path:

- `CreateFile(..., FILE_FLAG_BACKUP_SEMANTICS)` can open the Guest root directory.
- DOS `*.*` matches entries without a dot, so the engine now discovers `pac`.

The target consequently enumerates all 15 local YPF archives and opens the
script, image, voice, and update packs. Existing read-only handles now stream
from seekable Host files, avoiding an in-memory copy of the 1.6 GB `cg.ypf` and
keeping deep target runs around 100 MB RSS.

The unpacked child's ordinal imports exposed a correctness issue in synthetic
DLL export tables. COMCTL32 ordinal 17 and DSOUND ordinal 1 now occupy their
real export slots, allowing the target to cross main-window creation. ANSI
window captions use Windows-compatible replacement for malformed Shift-JIS
sequences rather than turning application bytes into a fatal Rust error.

The current executed path has crossed `CreateWindowExA` and reached real
resource keys such as `W0/CGS100`. Newly observed `FSQRT`, x87 arithmetic,
`SHLD`/`SHRD`, `JECXZ`, 64-bit `FISTP`, and implicit-`ST(0)` x87 memory
arithmetic are implemented with focused tests. The latest run reaches PNG
`gAMA` chunk processing from the real archives; `FDIV` and non-popping `FST`
cover the latest executed gamma conversion. No Guest D3D method has executed
yet; the immediate frontier remains image/resource initialization before
`Direct3DCreate9`.

The Host graphics substrate now uses wgpu 30 and has been verified locally to
select a real GPU, create an RGBA render-target texture, upload pixels, and
destroy it. D3D Guest interfaces will remain thin ABI/state adapters over this
shared backend, allowing later D3D8/D3D11 frontends to reuse Metal/Vulkan/D3D12
resource ownership. No native SDL3 window has been created yet.
The HD executable remains useful as a comparison path but is not the primary
target.
