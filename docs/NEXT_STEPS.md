# VNRT next steps

This plan optimizes for reaching the first real PE32 program with the smallest
amount of compatibility surface, rather than broad Windows coverage.

## Completed execution foundation

- Parse PE32 data directories, imports, and base relocation blocks.
- Translate RVAs through one bounds-checked section mapping implementation.
- Bind named imports into the guest IAT using reserved host-call thunk addresses.
- Reject ordinal-only imports explicitly until the registry gains ordinal keys.
- Read/write 32-bit register, immediate, and effective-address operands.
- Calculate `CF`, `PF`, `AF`, `ZF`, `SF`, and `OF` for arithmetic and logic.
- Execute `mov`, `lea`, arithmetic/logic, stack, call/return, and common branches.
- Create a one MiB guest stack with deterministic initial `ESP`.
- Run a complete synthetic PE32 from its entry point through an indirect IAT call
  to `kernel32!ExitProcess`, preserving the CPU-to-Win32 dependency boundary.
- Execute 8-bit and 16-bit register/memory operands, `movzx`, `movsx`, `cwde`,
  and left shifts while preserving partial-register semantics.
- Reproducibly build a freestanding C PE32 with Clang and bundled `rust-lld`,
  including a real `kernel32!ExitProcess` import and no Windows SDK dependency.
- Run that compiler-generated PE through `vnrt-runner` and propagate its exit
  code 42 back to the host process.
- Provide a monotonic process clock plus a page-backed guest heap with
  transactional unmapping.
- Implement `GetTickCount`, `GetProcessHeap`, `HeapAlloc`, and `HeapFree`; verify
  zero initialization, guest reads/writes, stdcall cleanup, and released pages
  from the compiler-generated fixture.
- Implement null-name `GetModuleHandleA/W` plus `VirtualAlloc` and `VirtualFree`
  for the common reserve+commit/release path, backed by a separate region
  allocator and verified by the C fixture.
- Capture failure EIP bytes, registers, and the last 16 Host Calls; print them
  from Runner on unsupported execution.
- Decode bounded Japanese Windows ANSI and UTF-16LE Guest strings.
- Maintain pseudo handles for Host DLLs and implement named
  `GetModuleHandleA/W` plus `GetProcAddress`; dynamically allocate and execute a
  new Host thunk for `GetTickCount` from the C fixture.
- Provide configurable process identity with persistent ANSI/UTF-16 command-line
  buffers, module paths, and simplified thread LastError state.
- Implement `GetCommandLineA/W`, `GetModuleFileNameA/W`, `GetLastError`, and
  `SetLastError`; verify both stack-array output encodings from compiled C.
- Add a root-confined Host filesystem, per-handle read cursors, and
  `CreateFileA/W`, `ReadFile`, and `CloseHandle`; reject absolute, drive, and
  parent-traversal paths.
- Read and validate a real sidecar resource through both ANSI and UTF-16 paths,
  close every handle, and execute a compiler-requested freestanding `memset`
  loop rather than avoiding CRT-shaped code.
- Implement `GetFileSize`, `GetStdHandle`, and standard-stream `WriteFile`;
  capture stdout/stderr in Runtime and forward Guest stdout through Runner.
- Add signed start/current/end file seeking and `GetFileType`; verify a negative
  end-relative seek followed by a second resource read.
- Add a case-insensitive Runtime environment and `GetEnvironmentVariableA/W`,
  including Win32 buffer-size semantics and missing-variable LastError.
- Enter through a compiler-generated C `mainCRTStartup`, with no assembly entry
  shim or synthetic halt instruction.
- Add a Win32-visible current directory and `GetCurrentDirectoryA/W`, keeping it
  separate from the Host filesystem sandbox root.
- Apply an x86 `FS:` segment base to memory operands, install minimal 32-bit
  TEB/PEB pages, and synchronize `fs:[34h]` with Kernel32 LastError calls.
- Expose deterministic process/thread IDs through both TEB ClientId fields and
  `GetCurrentProcessId`/`GetCurrentThreadId`.
- Run both unoptimized and scalar `-O1` compiler fixtures, including all
  `SETcc`/`CMOVcc` conditions and sign-extended 16-bit immediates.
- Populate normalized 32-bit `RTL_USER_PROCESS_PARAMETERS` with standard
  handles, current directory, image path, command line, and environment block;
  expose it through `PEB.ProcessParameters`.
- Share the same process environment through `GetEnvironmentStringsA/W` and
  `FreeEnvironmentStringsA/W` for CRT-style enumeration.
- Populate `PEB_LDR_DATA` and three circular doubly linked loader lists for the
  main image, kernel32, and user32, including normalized module names.
- Parse `IMAGE_TLS_DIRECTORY32`, copy the main image TLS template, apply zero
  fill, publish slot zero through `fs:[2Ch]`, and write the module TLS index.
- Execute null-terminated TLS callback arrays in image order with stdcall
  `(ImageBase, DLL_PROCESS_ATTACH, NULL)` frames before the PE entry point.
- Execute compiler-emitted `SHR`, `SAR`, `IMUL`, `MUL`, `DIV`, `IDIV`, `CDQ`,
  `CWD`, and `NEG`, including explicit divide faults and arithmetic flags.
- Execute `CLD`/`STD`, byte/word/dword `MOVS`, `STOS`, `LODS`, `CMPS`, and
  `SCAS`; REP iterations remain individually bounded by the Runtime step limit.
- Consume and produce carry correctly for byte/word/dword `ADC` and `SBB`,
  including `CF`, `PF`, `AF`, `ZF`, `SF`, and `OF`.
- Present a fixed Windows 10 22H2 Workstation identity (`10.0.19045`) through
  `GetVersion` and `GetVersionExA/W`, without legacy manifest-dependent version
  fallback behavior.
- Populate `STARTUPINFOA/W` with standard handles and implement
  `QueryPerformanceCounter`, `QueryPerformanceFrequency`, and
  `GetSystemTimeAsFileTime`.
- Bind PE32 ordinal imports to stable `dll!#ordinal` Host Call keys. Missing
  ordinal implementations now fail when called instead of blocking image load.
- Run the selected euphoria PE32 through CRT, CP932/NLS initialization, PE
  resources, real PAC directory enumeration, display probing, MCI cleanup, and
  a clean application shutdown path.
- Use a sub-page NT-style heap arena so small reads into mapped heap metadata do
  not fault merely because an allocation begins at a Host page boundary.
- Execute target-observed `LEAVE`, `NOT`, `XCHG`, EFLAGS probing, x87 control
  state, and the minimal MMX `MOVD/PUNPCKLDQ/MOVQ/EMMS` sequence.
- Replace per-access tree lookup with a sparse two-level page table, add
  page-generation-validated instruction decoding, batch untraced execution,
  and add safe single-page integer access fast paths.
- Cache decoded instructions and promote only repeatedly executed basic blocks;
  preserve page-generation invalidation for self-modifying unpacker code.
- Execute the target-observed x87 load/arithmetic/store sequence and `LOOP`
  control flow, while retaining a bounded control-transfer history for failures.
- Model `psapi.dll!GetModuleBaseNameA/W` and continue the unpacked child's
  Kernel32 export census through filesystem, time, locale, process, mapping,
  and initial thread probes.

## Next highest-leverage milestone: finish the child's import census

The primary target is now `euphoriaCN.exe`. Its self-unpacking path completes,
the NTDLL export census is resolved, and an unpacked child image is mapped near
`0x70100000`. The invalid indirect-target failure has been traced and removed.
Continue from the current `SetEndOfFile` export probe until the child
finishes Kernel32 resolution and enters engine initialization.

## Following target milestone: complete target and first real window

The import census has exposed thread and file-mapping APIs. Their current
facades distinguish export availability from runtime support, but real calls
must gain proper object lifetime and Guest scheduling semantics.

1. Finish target-driven Kernel32 resolution without broad speculative coverage.
2. Implement cooperative Guest thread contexts if `CreateThread` is actually
   invoked, including stacks, TLS, suspend counts, exit codes, and waits.
3. Grow the User32 surface only from executed failures until `CreateWindowExA`,
   then attach the first SDL3 native window.

## Following low-level milestones

### Process bootstrap

- Add a guard page below the committed guest stack.
- Define deterministic initial register state and entry-point calling contract.
- Add minimal SEH/TEB exception-chain support when the CRT or selected target
  first installs an exception handler.

### Memory management

- Separate address reservation from page commitment.
- Introduce a generation counter before exposing cached host pointers, so stale
  translations cannot survive unmap or protection changes.

### Loader completion

- Apply `IMAGE_REL_BASED_HIGHLOW` when the preferred image base is unavailable.
- Add ordinal API keys and delay-import parsing only when encountered in a target.
- Validate overlapping sections and alignment rules more deeply.

### Minimal Win32 path

- Add only the process APIs named by the compatibility census or CRT fixture.
- Make `MessageBoxA/W` the first SDL3-backed user-visible API after the headless
  bootstrap reaches it.

### Legacy game CPU path

- Begin x87 state and instruction support from instructions observed in the
  selected game; 32-bit visual novels commonly depend on compiler-generated
  floating-point code before SIMD becomes relevant.

## Explicit non-goals for now

- JIT compilation, x64 guests, drivers, COM, registry emulation, networking,
  Direct3D, broad GDI coverage, and general Windows application compatibility.
