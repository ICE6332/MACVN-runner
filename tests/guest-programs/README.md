# Guest programs

Place tiny, redistributable PE32 fixtures used by integration tests here. Do not
commit commercial game executables or assets.

`exit42.exe` contains a freestanding function compiled from `exit42.c` by Clang.
It performs byte/word operations, reads the process clock, obtains the main
module handle by name, resolves `GetTickCount` dynamically through
`GetProcAddress`, obtains ANSI/UTF-16 command lines, copies the module path into
stack arrays, round-trips LastError, allocates/frees heap and virtual memory,
and finally calls `kernel32!ExitProcess(42)`. The PE entry point is the
compiler-generated C function `mainCRTStartup`; no assembly shim or synthetic
HLT is involved.

The guest opens `resource.txt` through both `CreateFileA` and `CreateFileW`,
performs cursor-based `ReadFile` calls, validates the bytes, and closes both
handles. Runner exposes only the executable's containing directory as the file
root.

The guest also checks `GetFileSize`, obtains stdout from `GetStdHandle`, and
writes `guest-ok` through `WriteFile`; Runner forwards those captured bytes to
the host terminal.

It performs a negative end-relative `SetFilePointer` seek, checks disk/character
handle types, and reads the case-insensitive `VNRT_TEST=ready` environment value
through both ANSI and UTF-16 APIs. It also reads the Win32-visible current
directory through `GetCurrentDirectoryA/W`.

Compiler-generated `FS:` accesses verify the minimal TEB/PEB self links, main
image base, and two-way synchronization between `fs:[34h]` and
`GetLastError`/`SetLastError`.
It also cross-checks TEB ClientId values against
`GetCurrentProcessId`/`GetCurrentThreadId`.
The Guest follows `PEB.ProcessParameters` directly and verifies its standard
handles, current directory, image path, command line, environment block, and
the PEB process-heap handle.
It enumerates that same environment through ANSI and UTF-16
`GetEnvironmentStrings` calls and releases both views through the matching API.
Finally, it traverses all three `PEB_LDR_DATA` lists, validates every forward
and backward link, and finds the EXE, kernel32, and user32 module bases without
using a module-enumeration API.
The image also carries a real PE32 TLS Directory. Guest code reads its assigned
slot through `fs:[2Ch]`, checks copied initialized data and zero fill, then
mutates the thread-local allocation to prove the slot remains stable.
Two stdcall TLS callbacks run before `mainCRTStartup`, validate their arguments
and TLS visibility, and record their execution order for the entry point.
Volatile runtime inputs drive variable `IMUL`, `DIV`, `IDIV`, `SHR`, and `SAR`
operations in both optimization modes, with exact quotient, remainder, shift,
and negated-result checks.
Inline compiler functions exercise forward/reverse REP copying, dword filling,
byte scanning, early comparison termination, and accumulator loading on real
stack arrays.
The Guest also validates the runtime's fixed Windows 10 22H2 Workstation
identity (`10.0.19045`), ANSI/UTF-16 `STARTUPINFO`, high-resolution performance
counter, and Win32 `FILETIME` clock through real kernel32 imports.

Rebuild it on a host with Clang and the pinned Rust toolchain:

```sh
./tests/guest-programs/build-exit42.sh
```

The script uses the `rust-lld` bundled with Rust. It temporarily builds a stub
`kernel32.dll` to produce the import library, then deletes the DLL, library, and
object files. `kernel32.def` keeps the guest import name compatible with the real
Windows API despite the i686 stdcall symbol decoration. No Windows SDK or MinGW
installation is required.

The script also emits `exit42-opt.exe` at `-O1` with SSE disabled. This fixture
makes compiler-emitted `SETcc`, `CMOVcc`, and compact sign-extended immediates
part of the required scalar interpreter surface while leaving SIMD as a
separate milestone.
