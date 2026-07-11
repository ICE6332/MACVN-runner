//! Initial `kernel32.dll` host-call registrations.

use encoding_rs::{Encoding, SHIFT_JIS, UTF_8};
use tracing::debug;
use vnrt_win32::{
    ApiKey, ApiRegistry, FileEntry, GuestAddress, Handle, HostCallContext, HostCallHandler,
    MAX_GUEST_STRING_BYTES, PROCESS_HEAP_HANDLE, Win32Error, encode_ansi_z, encode_utf16_z,
    read_ansi_z, read_utf16_z,
};

const MODULE: &str = "kernel32.dll";
const HEAP_NO_SERIALIZE: u32 = 0x0000_0001;
const HEAP_GENERATE_EXCEPTIONS: u32 = 0x0000_0004;
const HEAP_CREATE_ENABLE_EXECUTE: u32 = 0x0004_0000;
const HEAP_ZERO_MEMORY: u32 = 0x0000_0008;
const MEM_COMMIT: u32 = 0x0000_1000;
const MEM_RESERVE: u32 = 0x0000_2000;
const MEM_COMMIT_RESERVE: u32 = MEM_COMMIT | MEM_RESERVE;
const MEM_RELEASE: u32 = 0x0000_8000;
const GENERIC_READ: u32 = 0x8000_0000;
const OPEN_EXISTING: u32 = 3;
const INVALID_HANDLE_VALUE: u32 = u32::MAX;

/// Register the current `kernel32.dll` surface.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "ExitProcess"), ExitProcess);
    registry.register(ApiKey::new(MODULE, "GetTickCount"), GetTickCount);
    registry.register(ApiKey::new(MODULE, "GetStartupInfoA"), GetStartupInfo);
    registry.register(ApiKey::new(MODULE, "GetStartupInfoW"), GetStartupInfo);
    registry.register(ApiKey::new(MODULE, "GetVersion"), GetVersion);
    registry.register(
        ApiKey::new(MODULE, "GetVersionExA"),
        GetVersionEx { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetVersionExW"),
        GetVersionEx { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "QueryPerformanceCounter"),
        QueryPerformance { frequency: false },
    );
    registry.register(
        ApiKey::new(MODULE, "QueryPerformanceFrequency"),
        QueryPerformance { frequency: true },
    );
    registry.register(
        ApiKey::new(MODULE, "GetSystemTimeAsFileTime"),
        GetSystemTimeAsFileTime,
    );
    registry.register(ApiKey::new(MODULE, "GetProcessHeap"), GetProcessHeap);
    registry.register(ApiKey::new(MODULE, "HeapCreate"), HeapCreate);
    registry.register(ApiKey::new(MODULE, "HeapDestroy"), HeapDestroy);
    registry.register(ApiKey::new(MODULE, "HeapAlloc"), HeapAlloc);
    registry.register(ApiKey::new(MODULE, "HeapReAlloc"), HeapReAlloc);
    registry.register(ApiKey::new(MODULE, "HeapFree"), HeapFree);
    registry.register(ApiKey::new(MODULE, "HeapSize"), HeapSize);
    registry.register(ApiKey::new(MODULE, "GlobalAlloc"), GlobalAlloc);
    registry.register(ApiKey::new(MODULE, "GlobalLock"), GlobalLock);
    registry.register(ApiKey::new(MODULE, "GlobalUnlock"), GlobalUnlock);
    registry.register(ApiKey::new(MODULE, "GlobalFree"), GlobalFree);
    registry.register(ApiKey::new(MODULE, "LocalAlloc"), LocalAlloc);
    registry.register(ApiKey::new(MODULE, "LocalFree"), LocalFree);
    registry.register(
        ApiKey::new(MODULE, "InitializeCriticalSection"),
        CriticalSection {
            operation: CriticalSectionOperation::Initialize,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "InitializeCriticalSectionAndSpinCount"),
        CriticalSection {
            operation: CriticalSectionOperation::InitializeAndSpinCount,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "DeleteCriticalSection"),
        CriticalSection {
            operation: CriticalSectionOperation::Delete,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "EnterCriticalSection"),
        CriticalSection {
            operation: CriticalSectionOperation::Enter,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "LeaveCriticalSection"),
        CriticalSection {
            operation: CriticalSectionOperation::Leave,
        },
    );
    registry.register(ApiKey::new(MODULE, "TlsAlloc"), TlsAlloc);
    registry.register(ApiKey::new(MODULE, "TlsFree"), TlsFree);
    registry.register(ApiKey::new(MODULE, "TlsGetValue"), TlsGetValue);
    registry.register(ApiKey::new(MODULE, "TlsSetValue"), TlsSetValue);
    registry.register(ApiKey::new(MODULE, "SetHandleCount"), SetHandleCount);
    registry.register(
        ApiKey::new(MODULE, "WideCharToMultiByte"),
        WideCharToMultiByte,
    );
    registry.register(
        ApiKey::new(MODULE, "MultiByteToWideChar"),
        MultiByteToWideChar,
    );
    registry.register(ApiKey::new(MODULE, "GetACP"), GetCodePage);
    registry.register(ApiKey::new(MODULE, "GetOEMCP"), GetCodePage);
    registry.register(ApiKey::new(MODULE, "GetCPInfo"), GetCpInfo);
    registry.register(ApiKey::new(MODULE, "GetSystemInfo"), GetSystemInfo);
    registry.register(
        ApiKey::new(MODULE, "GetStringTypeW"),
        GetStringType { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "GetStringTypeA"),
        GetStringType { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "LCMapStringW"),
        LcMapString { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "LCMapStringA"),
        LcMapString { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "IsProcessorFeaturePresent"),
        IsProcessorFeaturePresent,
    );
    registry.register(
        ApiKey::new(MODULE, "SetUnhandledExceptionFilter"),
        SetUnhandledExceptionFilter,
    );
    registry.register(
        ApiKey::new(MODULE, "UnhandledExceptionFilter"),
        UnhandledExceptionFilter,
    );
    registry.register(ApiKey::new(MODULE, "CreateMutexA"), CreateMutexA);
    registry.register(ApiKey::new(MODULE, "ReleaseMutex"), ReleaseMutex);
    registry.register(ApiKey::new(MODULE, "CreateEventA"), CreateEventA);
    registry.register(ApiKey::new(MODULE, "SetEvent"), SetEvent { signaled: true });
    registry.register(
        ApiKey::new(MODULE, "ResetEvent"),
        SetEvent { signaled: false },
    );
    registry.register(
        ApiKey::new(MODULE, "WaitForSingleObject"),
        WaitForObjects { multiple: false },
    );
    registry.register(
        ApiKey::new(MODULE, "WaitForMultipleObjects"),
        WaitForObjects { multiple: true },
    );
    registry.register(
        ApiKey::new(MODULE, "GetModuleHandleA"),
        GetModuleHandle { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetModuleHandleW"),
        GetModuleHandle { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "LoadLibraryA"),
        LoadLibrary { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "LoadLibraryW"),
        LoadLibrary { wide: true },
    );
    registry.register(ApiKey::new(MODULE, "EncodePointer"), PointerCodec);
    registry.register(ApiKey::new(MODULE, "DecodePointer"), PointerCodec);
    for (name, operation) in [
        ("InterlockedIncrement", InterlockedOperation::Increment),
        ("InterlockedDecrement", InterlockedOperation::Decrement),
        ("InterlockedExchange", InterlockedOperation::Exchange),
        (
            "InterlockedCompareExchange",
            InterlockedOperation::CompareExchange,
        ),
        ("InterlockedExchangeAdd", InterlockedOperation::ExchangeAdd),
    ] {
        registry.register(ApiKey::new(MODULE, name), Interlocked { operation });
    }
    registry.register(ApiKey::new(MODULE, "GetProcAddress"), GetProcAddress);
    registry.register(
        ApiKey::new(MODULE, "GetCommandLineA"),
        GetCommandLine { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetCommandLineW"),
        GetCommandLine { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "GetModuleFileNameA"),
        GetModuleFileName { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetModuleFileNameW"),
        GetModuleFileName { wide: true },
    );
    registry.register(ApiKey::new(MODULE, "GetLastError"), GetLastError);
    registry.register(ApiKey::new(MODULE, "SetLastError"), SetLastError);
    registry.register(ApiKey::new(MODULE, "VirtualAlloc"), VirtualAlloc);
    registry.register(ApiKey::new(MODULE, "VirtualFree"), VirtualFree);
    registry.register(ApiKey::new(MODULE, "VirtualProtect"), VirtualProtect);
    registry.register(
        ApiKey::new(MODULE, "CreateFileA"),
        CreateFile { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "CreateFileW"),
        CreateFile { wide: true },
    );
    registry.register(ApiKey::new(MODULE, "ReadFile"), ReadFile);
    registry.register(ApiKey::new(MODULE, "FindFirstFileA"), FindFirstFileA);
    registry.register(ApiKey::new(MODULE, "FindNextFileA"), FindNextFileA);
    registry.register(ApiKey::new(MODULE, "FindClose"), FindClose);
    registry.register(ApiKey::new(MODULE, "FindResourceA"), FindResourceA);
    registry.register(ApiKey::new(MODULE, "SizeofResource"), SizeofResource);
    registry.register(ApiKey::new(MODULE, "LoadResource"), LoadResource);
    registry.register(ApiKey::new(MODULE, "LockResource"), LockResource);
    registry.register(ApiKey::new(MODULE, "FreeResource"), FreeResource);
    registry.register(ApiKey::new(MODULE, "CloseHandle"), CloseHandle);
    registry.register(ApiKey::new(MODULE, "GetStdHandle"), GetStdHandle);
    registry.register(ApiKey::new(MODULE, "WriteFile"), WriteFile);
    registry.register(ApiKey::new(MODULE, "GetFileSize"), GetFileSize);
    registry.register(ApiKey::new(MODULE, "SetFilePointer"), SetFilePointer);
    registry.register(ApiKey::new(MODULE, "GetFileType"), GetFileType);
    registry.register(
        ApiKey::new(MODULE, "GetEnvironmentVariableA"),
        GetEnvironmentVariable { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetEnvironmentVariableW"),
        GetEnvironmentVariable { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "GetEnvironmentStringsA"),
        GetEnvironmentStrings { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetEnvironmentStringsW"),
        GetEnvironmentStrings { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "FreeEnvironmentStringsA"),
        FreeEnvironmentStrings { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "FreeEnvironmentStringsW"),
        FreeEnvironmentStrings { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "GetCurrentDirectoryA"),
        GetCurrentDirectory { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetCurrentDirectoryW"),
        GetCurrentDirectory { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "SetCurrentDirectoryA"),
        SetCurrentDirectoryA,
    );
    registry.register(
        ApiKey::new(MODULE, "GetCurrentProcessId"),
        CurrentId { thread: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetCurrentThreadId"),
        CurrentId { thread: true },
    );
    registry.register(
        ApiKey::new(MODULE, "GetCurrentProcess"),
        CurrentPseudoHandle { thread: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetCurrentThread"),
        CurrentPseudoHandle { thread: true },
    );
    registry.register(
        ApiKey::new(MODULE, "GetThreadPreferredUILanguages"),
        GetThreadPreferredUiLanguages,
    );
    registry.register(
        ApiKey::new(MODULE, "GetFullPathNameA"),
        GetFullPathName { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetFullPathNameW"),
        GetFullPathName { wide: true },
    );
    registry.register(ApiKey::new(MODULE, "GetFileTime"), GetFileTime);
    registry.register(
        ApiKey::new(MODULE, "OutputDebugStringA"),
        OutputDebugString { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "OutputDebugStringW"),
        OutputDebugString { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "GetShortPathNameA"),
        GetShortPathName { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetShortPathNameW"),
        GetShortPathName { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "GetLongPathNameA"),
        GetShortPathName { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetLongPathNameW"),
        GetShortPathName { wide: true },
    );
}

#[derive(Debug, Clone, Copy)]
struct ExitProcess;

impl HostCallHandler for ExitProcess {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let exit_code = context.argument_u32(0)?;
        context.request_exit(exit_code);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetTickCount;

impl HostCallHandler for GetTickCount {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.set_return_u32(context.tick_count());
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetStartupInfo;

impl HostCallHandler for GetStartupInfo {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        const STARTUP_INFO_SIZE: usize = 68;
        const STARTF_USESTDHANDLES: u32 = 0x0000_0100;
        let output = GuestAddress(context.argument_u32(0)?);
        let mut bytes = vec![0; STARTUP_INFO_SIZE];
        put_u32(&mut bytes, 0x00, STARTUP_INFO_SIZE as u32)?;
        put_u32(&mut bytes, 0x2c, STARTF_USESTDHANDLES)?;
        put_u32(
            &mut bytes,
            0x38,
            context
                .standard_handle(-10)
                .ok_or(Win32Error::InvalidArgument("missing standard input handle"))?
                .0,
        )?;
        put_u32(
            &mut bytes,
            0x3c,
            context
                .standard_handle(-11)
                .ok_or(Win32Error::InvalidArgument(
                    "missing standard output handle",
                ))?
                .0,
        )?;
        put_u32(
            &mut bytes,
            0x40,
            context
                .standard_handle(-12)
                .ok_or(Win32Error::InvalidArgument("missing standard error handle"))?
                .0,
        )?;
        context.write_memory(output, &bytes)?;
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetVersion;

impl HostCallHandler for GetVersion {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        // Windows 10 22H2 compatibility identity: major/minor in the low word,
        // build number in the high word, with the Win32 NT high bit clear.
        context.set_return_u32((19_045 << 16) | 10);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetVersionEx {
    wide: bool,
}

impl HostCallHandler for GetVersionEx {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let output = GuestAddress(context.argument_u32(0)?);
        let mut size_bytes = [0; 4];
        context.read_memory(output, &mut size_bytes)?;
        let size = u32::from_le_bytes(size_bytes);
        let (basic_size, extended_size) = if self.wide {
            (276_u32, 284_u32)
        } else {
            (148_u32, 156_u32)
        };
        if size != basic_size && size != extended_size {
            context.set_last_error(87); // ERROR_INVALID_PARAMETER
            context.set_return_u32(0);
            context.set_stdcall_cleanup(4);
            return Ok(());
        }
        let mut bytes = vec![
            0;
            usize::try_from(size).map_err(|_| {
                Win32Error::InvalidArgument("OSVERSIONINFO size does not fit host usize")
            })?
        ];
        put_u32(&mut bytes, 0x00, size)?;
        put_u32(&mut bytes, 0x04, 10)?;
        put_u32(&mut bytes, 0x08, 0)?;
        put_u32(&mut bytes, 0x0c, 19_045)?;
        put_u32(&mut bytes, 0x10, 2)?; // VER_PLATFORM_WIN32_NT
        if size == extended_size {
            put_u16(&mut bytes, basic_size as usize, 0)?;
            put_u16(&mut bytes, basic_size as usize + 2, 0)?;
            put_u16(&mut bytes, basic_size as usize + 4, 0)?;
            let product_type =
                bytes
                    .get_mut(basic_size as usize + 6)
                    .ok_or(Win32Error::InvalidArgument(
                        "OSVERSIONINFOEX product type offset",
                    ))?;
            *product_type = 1; // VER_NT_WORKSTATION
        }
        context.write_memory(output, &bytes)?;
        context.set_last_error(0);
        context.set_return_u32(1);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct QueryPerformance {
    frequency: bool,
}

impl HostCallHandler for QueryPerformance {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let output = GuestAddress(context.argument_u32(0)?);
        if output.0 == 0 {
            context.set_last_error(87);
            context.set_return_u32(0);
            context.set_stdcall_cleanup(4);
            return Ok(());
        }
        let value = if self.frequency {
            context.performance_frequency()
        } else {
            context.performance_counter()
        };
        context.write_memory(output, &value.to_le_bytes())?;
        context.set_last_error(0);
        context.set_return_u32(1);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetSystemTimeAsFileTime;

impl HostCallHandler for GetSystemTimeAsFileTime {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let output = GuestAddress(context.argument_u32(0)?);
        context.write_memory(output, &context.system_time_filetime().to_le_bytes())?;
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetProcessHeap;

impl HostCallHandler for GetProcessHeap {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.set_return_u32(PROCESS_HEAP_HANDLE);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct HeapCreate;

impl HostCallHandler for HeapCreate {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let options = context.argument_u32(0)?;
        if options & !(HEAP_NO_SERIALIZE | HEAP_CREATE_ENABLE_EXECUTE) != 0 {
            return Err(Win32Error::Unsupported {
                feature: "kernel32!HeapCreate options",
            });
        }
        let heap = context.create_heap(
            context.argument_u32(1)?,
            context.argument_u32(2)?,
            options & HEAP_CREATE_ENABLE_EXECUTE != 0,
        )?;
        context.set_return_u32(heap.0);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct HeapDestroy;

impl HostCallHandler for HeapDestroy {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.destroy_heap(Handle(context.argument_u32(0)?))?;
        context.set_return_u32(1);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct HeapAlloc;

impl HostCallHandler for HeapAlloc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let heap = Handle(context.argument_u32(0)?);
        let flags = context.argument_u32(1)?;
        if flags & !(HEAP_NO_SERIALIZE | HEAP_GENERATE_EXCEPTIONS | HEAP_ZERO_MEMORY) != 0 {
            return Err(Win32Error::Unsupported {
                feature: "kernel32!HeapAlloc flags",
            });
        }
        let size = context.argument_u32(2)?;
        let address = context.allocate_heap_memory(heap, size)?;
        context.set_return_u32(address.0);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct HeapReAlloc;

impl HostCallHandler for HeapReAlloc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let heap = Handle(context.argument_u32(0)?);
        let flags = context.argument_u32(1)?;
        if flags & !(HEAP_NO_SERIALIZE | HEAP_GENERATE_EXCEPTIONS | HEAP_ZERO_MEMORY) != 0 {
            return Err(Win32Error::Unsupported {
                feature: "kernel32!HeapReAlloc flags",
            });
        }
        let address = GuestAddress(context.argument_u32(2)?);
        let replacement =
            context.reallocate_heap_memory(heap, address, context.argument_u32(3)?)?;
        context.set_return_u32(replacement.0);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct HeapFree;

impl HostCallHandler for HeapFree {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let heap = Handle(context.argument_u32(0)?);
        if context.argument_u32(1)? & !HEAP_NO_SERIALIZE != 0 {
            return Err(Win32Error::Unsupported {
                feature: "kernel32!HeapFree nonzero flags",
            });
        }
        let address = GuestAddress(context.argument_u32(2)?);
        context.free_heap_memory(heap, address)?;
        context.set_return_u32(1);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct HeapSize;

impl HostCallHandler for HeapSize {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let heap = Handle(context.argument_u32(0)?);
        if context.argument_u32(1)? & !HEAP_NO_SERIALIZE != 0 {
            return Err(Win32Error::Unsupported {
                feature: "kernel32!HeapSize nonzero flags",
            });
        }
        let size = context.heap_memory_size(heap, GuestAddress(context.argument_u32(2)?))?;
        context.set_return_u32(size);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GlobalAlloc;

impl HostCallHandler for GlobalAlloc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let flags = context.argument_u32(0)?;
        if flags & !(0x0002 | 0x0040 | 0x2000) != 0 {
            return Err(Win32Error::Unsupported {
                feature: "GlobalAlloc flags",
            });
        }
        let size = context.argument_u32(1)?;
        let handle = context.allocate_global_memory(size)?;
        context.set_return_u32(handle.0);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GlobalLock;

impl HostCallHandler for GlobalLock {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        match context.lock_global_memory(handle) {
            Ok(address) => context.set_return_u32(address.0),
            Err(_) => {
                context.set_last_error(6);
                context.set_return_u32(0);
            }
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GlobalUnlock;

impl HostCallHandler for GlobalUnlock {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        match context.unlock_global_memory(handle) {
            Ok(still_locked) => {
                context.set_last_error(0);
                context.set_return_u32(u32::from(still_locked));
            }
            Err(_) => {
                context.set_last_error(158); // ERROR_NOT_LOCKED
                context.set_return_u32(0);
            }
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GlobalFree;

#[derive(Debug, Clone, Copy)]
struct LocalAlloc;

#[derive(Debug, Clone, Copy)]
struct LocalFree;

impl HostCallHandler for LocalAlloc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let flags = context.argument_u32(0)?;
        if flags & !0x0040 != 0 {
            return Err(Win32Error::Unsupported {
                feature: "LocalAlloc movable or unsupported flags",
            });
        }
        let address =
            context.allocate_heap_memory(Handle(PROCESS_HEAP_HANDLE), context.argument_u32(1)?)?;
        context.set_return_u32(address.0);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for LocalFree {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let address = GuestAddress(context.argument_u32(0)?);
        if address.0 == 0
            || context
                .free_heap_memory(Handle(PROCESS_HEAP_HANDLE), address)
                .is_ok()
        {
            context.set_return_u32(0);
        } else {
            context.set_last_error(6); // ERROR_INVALID_HANDLE
            context.set_return_u32(address.0);
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for GlobalFree {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        if context.free_global_memory(handle).is_ok() {
            context.set_last_error(0);
            context.set_return_u32(0);
        } else {
            context.set_last_error(6);
            context.set_return_u32(handle.0);
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum CriticalSectionOperation {
    Initialize,
    InitializeAndSpinCount,
    Delete,
    Enter,
    Leave,
}

#[derive(Debug, Clone, Copy)]
struct CriticalSection {
    operation: CriticalSectionOperation,
}

#[derive(Debug, Clone, Copy)]
struct TlsAlloc;

impl HostCallHandler for TlsAlloc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        match context.allocate_tls_index() {
            Ok(index) => {
                context.set_last_error(0);
                context.set_return_u32(index);
            }
            Err(Win32Error::OutOfMemory) => {
                context.set_last_error(8); // ERROR_NOT_ENOUGH_MEMORY
                context.set_return_u32(u32::MAX); // TLS_OUT_OF_INDEXES
            }
            Err(error) => return Err(error),
        }
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct TlsFree;

impl HostCallHandler for TlsFree {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let success = context.free_tls_index(context.argument_u32(0)?).is_ok();
        context.set_last_error(if success { 0 } else { 87 });
        context.set_return_u32(u32::from(success));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct TlsGetValue;

impl HostCallHandler for TlsGetValue {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        match context.tls_value(context.argument_u32(0)?) {
            Ok(value) => {
                context.set_last_error(0);
                context.set_return_u32(value);
            }
            Err(_) => {
                context.set_last_error(87);
                context.set_return_u32(0);
            }
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct TlsSetValue;

impl HostCallHandler for TlsSetValue {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let success = context
            .set_tls_value(context.argument_u32(0)?, context.argument_u32(1)?)
            .is_ok();
        context.set_last_error(if success { 0 } else { 87 });
        context.set_return_u32(u32::from(success));
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SetHandleCount;

impl HostCallHandler for SetHandleCount {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        // NT-based Windows keeps this legacy DOS compatibility API as a no-op.
        context.set_return_u32(context.argument_u32(0)?);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct WideCharToMultiByte;

impl HostCallHandler for WideCharToMultiByte {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let encoding = windows_encoding(context.argument_u32(0)?)?;
        let flags = context.argument_u32(1)?;
        if flags & !(0x10 | 0x20 | 0x40 | 0x80 | 0x200 | 0x400) != 0 {
            return Err(Win32Error::Unsupported {
                feature: "WideCharToMultiByte conversion flags",
            });
        }
        let input = GuestAddress(context.argument_u32(2)?);
        let input_length = context.argument_u32(3)? as i32;
        let (text, include_nul) = if input_length == -1 {
            (read_utf16_z(context, input)?, true)
        } else if input_length > 0 {
            let unit_count = usize::try_from(input_length)
                .map_err(|_| Win32Error::InvalidArgument("wide input length"))?;
            let bytes = read_guest_bytes(context, input, unit_count.saturating_mul(2))?;
            let units = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            (
                String::from_utf16(&units)
                    .map_err(|_| Win32Error::InvalidArgument("invalid UTF-16 input"))?,
                false,
            )
        } else {
            return Err(Win32Error::InvalidArgument("wide input length"));
        };
        let (encoded, _, used_default) = encoding.encode(&text);
        let mut output = encoded.into_owned();
        if include_nul {
            output.push(0);
        }
        let output_address = GuestAddress(context.argument_u32(4)?);
        let output_capacity = context.argument_u32(5)?;
        let used_default_address = GuestAddress(context.argument_u32(7)?);
        if used_default_address.0 != 0 {
            context.write_memory(used_default_address, &u32::from(used_default).to_le_bytes())?;
        }
        finish_conversion_bytes(context, output_address, output_capacity, &output, 32)
    }
}

#[derive(Debug, Clone, Copy)]
struct MultiByteToWideChar;

impl HostCallHandler for MultiByteToWideChar {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let encoding = windows_encoding(context.argument_u32(0)?)?;
        let flags = context.argument_u32(1)?;
        if flags & !(0x0000_0001 | 0x0000_0008) != 0 {
            return Err(Win32Error::Unsupported {
                feature: "MultiByteToWideChar conversion flags",
            });
        }
        let input = GuestAddress(context.argument_u32(2)?);
        let input_length = context.argument_u32(3)? as i32;
        let (bytes, include_nul) = if input_length == -1 {
            (read_guest_z_bytes(context, input)?, true)
        } else if input_length > 0 {
            (
                read_guest_bytes(
                    context,
                    input,
                    usize::try_from(input_length)
                        .map_err(|_| Win32Error::InvalidArgument("multibyte input length"))?,
                )?,
                false,
            )
        } else {
            return Err(Win32Error::InvalidArgument("multibyte input length"));
        };
        let (decoded, had_errors) = encoding.decode_without_bom_handling(&bytes);
        if had_errors && flags & 0x0000_0008 != 0 {
            context.set_last_error(1113); // ERROR_NO_UNICODE_TRANSLATION
            context.set_return_u32(0);
            context.set_stdcall_cleanup(24);
            return Ok(());
        }
        let mut units = decoded.encode_utf16().collect::<Vec<_>>();
        if include_nul {
            units.push(0);
        }
        let required = u32::try_from(units.len()).map_err(|_| Win32Error::OutOfMemory)?;
        let output = GuestAddress(context.argument_u32(4)?);
        let capacity = context.argument_u32(5)?;
        if capacity == 0 {
            context.set_last_error(0);
            context.set_return_u32(required);
        } else if capacity < required || output.0 == 0 {
            context.set_last_error(122); // ERROR_INSUFFICIENT_BUFFER
            context.set_return_u32(0);
        } else {
            let bytes = units
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            context.write_memory(output, &bytes)?;
            context.set_last_error(0);
            context.set_return_u32(required);
        }
        context.set_stdcall_cleanup(24);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetCodePage;

impl HostCallHandler for GetCodePage {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.set_return_u32(932);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetCpInfo;

impl HostCallHandler for GetCpInfo {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let code_page = match context.argument_u32(0)? {
            0 | 3 | 932 => 932,
            65_001 => 65_001,
            _ => {
                context.set_last_error(87);
                context.set_return_u32(0);
                context.set_stdcall_cleanup(8);
                return Ok(());
            }
        };
        let mut info = [0; 18];
        put_u32(&mut info, 0, if code_page == 932 { 2 } else { 4 })?;
        info[4] = b'?';
        if code_page == 932 {
            info[6..12].copy_from_slice(&[0x81, 0x9f, 0xe0, 0xfc, 0, 0]);
        }
        context.write_memory(GuestAddress(context.argument_u32(1)?), &info)?;
        context.set_last_error(0);
        context.set_return_u32(1);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetSystemInfo;

impl HostCallHandler for GetSystemInfo {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let mut info = [0; 36];
        put_u16(&mut info, 0, 0)?; // PROCESSOR_ARCHITECTURE_INTEL
        put_u32(&mut info, 4, 4096)?;
        put_u32(&mut info, 8, 0x0001_0000)?;
        put_u32(&mut info, 12, 0x7ffe_ffff)?;
        put_u32(&mut info, 16, 1)?;
        put_u32(&mut info, 20, 1)?;
        put_u32(&mut info, 24, 586)?; // PROCESSOR_INTEL_PENTIUM
        put_u32(&mut info, 28, 65_536)?;
        put_u16(&mut info, 32, 6)?;
        context.write_memory(GuestAddress(context.argument_u32(0)?), &info)?;
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetStringType {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct LcMapString {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct IsProcessorFeaturePresent;

impl HostCallHandler for IsProcessorFeaturePresent {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        // Advertise only the scalar baseline the interpreter actually executes.
        // Returning false is the Windows contract for unknown feature IDs too.
        let _feature = context.argument_u32(0)?;
        context.set_return_u32(0);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SetUnhandledExceptionFilter;

impl HostCallHandler for SetUnhandledExceptionFilter {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let previous = context.replace_unhandled_exception_filter(context.argument_u32(0)?);
        context.set_return_u32(previous);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct UnhandledExceptionFilter;

impl HostCallHandler for UnhandledExceptionFilter {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let exception_pointers = context.argument_u32(0)?;
        let filter = context.unhandled_exception_filter();
        context.set_stdcall_cleanup(4);
        if filter.0 == 0 {
            context.set_return_u32(0); // EXCEPTION_CONTINUE_SEARCH
            return Ok(());
        }
        context.request_guest_callback(filter, &[exception_pointers])?;
        context.use_guest_callback_return_value();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct CreateMutexA;

impl HostCallHandler for CreateMutexA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        if context.argument_u32(0)? != 0 {
            return Err(Win32Error::Unsupported {
                feature: "CreateMutexA security attributes",
            });
        }
        let initial_owner = context.argument_u32(1)? != 0;
        let name_address = GuestAddress(context.argument_u32(2)?);
        let name = (name_address.0 != 0)
            .then(|| read_ansi_z(context, name_address))
            .transpose()?;
        let (handle, already_existed) = context.create_mutex(name.as_deref(), initial_owner)?;
        context.set_last_error(if already_existed { 183 } else { 0 });
        context.set_return_u32(handle.0);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct ReleaseMutex;

impl HostCallHandler for ReleaseMutex {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let success = context
            .release_mutex(Handle(context.argument_u32(0)?))
            .is_ok();
        context.set_last_error(if success { 0 } else { 288 });
        context.set_return_u32(u32::from(success));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct CreateEventA;

impl HostCallHandler for CreateEventA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        if context.argument_u32(0)? != 0 {
            return Err(Win32Error::Unsupported {
                feature: "CreateEventA security attributes",
            });
        }
        let manual_reset = context.argument_u32(1)? != 0;
        let initial_state = context.argument_u32(2)? != 0;
        let name_address = GuestAddress(context.argument_u32(3)?);
        let name = (name_address.0 != 0)
            .then(|| read_ansi_z(context, name_address))
            .transpose()?;
        let (handle, already_existed) =
            context.create_event(name.as_deref(), manual_reset, initial_state)?;
        context.set_last_error(if already_existed { 183 } else { 0 });
        context.set_return_u32(handle.0);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SetEvent {
    signaled: bool,
}

impl HostCallHandler for SetEvent {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let success = context
            .set_event_state(Handle(context.argument_u32(0)?), self.signaled)
            .is_ok();
        context.set_last_error(if success { 0 } else { 6 });
        context.set_return_u32(u32::from(success));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct WaitForObjects {
    multiple: bool,
}

impl HostCallHandler for WaitForObjects {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let (handles, wait_all, timeout, cleanup) = if self.multiple {
            let count = context.argument_u32(0)?;
            if count == 0 || count > 64 {
                return Err(Win32Error::InvalidArgument("wait object count"));
            }
            let array = GuestAddress(context.argument_u32(1)?);
            let byte_length = usize::try_from(count)
                .ok()
                .and_then(|count| count.checked_mul(4))
                .ok_or(Win32Error::InvalidArgument("wait handle array size"))?;
            let mut bytes = vec![0; byte_length];
            context.read_memory(array, &mut bytes)?;
            let handles = bytes
                .chunks_exact(4)
                .map(|bytes| Handle(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])))
                .collect::<Vec<_>>();
            (
                handles,
                context.argument_u32(2)? != 0,
                context.argument_u32(3)?,
                16,
            )
        } else {
            (
                vec![Handle(context.argument_u32(0)?)],
                true,
                context.argument_u32(1)?,
                8,
            )
        };
        match context.try_wait_for_objects(&handles, wait_all) {
            Ok(Some(index)) => {
                context.set_last_error(0);
                context.set_return_u32(index);
            }
            Ok(None) if timeout == 0 => {
                context.set_last_error(0);
                context.set_return_u32(258);
            }
            Ok(None) => {
                return Err(Win32Error::Unsupported {
                    feature: "blocking wait without a Guest thread scheduler",
                });
            }
            Err(_) => {
                context.set_last_error(6);
                context.set_return_u32(u32::MAX);
            }
        }
        context.set_stdcall_cleanup(cleanup);
        Ok(())
    }
}

impl HostCallHandler for LcMapString {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let flags = context.argument_u32(1)?;
        if !matches!(flags, 0x0000_0100 | 0x0000_0200) {
            return Err(Win32Error::Unsupported {
                feature: "LCMapString mapping flags",
            });
        }
        let source = GuestAddress(context.argument_u32(2)?);
        let length = context.argument_u32(3)? as i32;
        let text = if self.wide {
            String::from_utf16(&read_guest_utf16_units(context, source, length)?)
                .map_err(|_| Win32Error::InvalidArgument("LCMapString UTF-16 input"))?
        } else {
            let bytes = if length < 0 {
                let mut bytes = read_guest_z_bytes(context, source)?;
                bytes.push(0);
                bytes
            } else if length > 0 {
                read_guest_bytes(
                    context,
                    source,
                    usize::try_from(length)
                        .map_err(|_| Win32Error::InvalidArgument("LCMapString ANSI length"))?,
                )?
            } else {
                return Err(Win32Error::InvalidArgument("LCMapString source length"));
            };
            SHIFT_JIS.decode_without_bom_handling(&bytes).0.into_owned()
        };
        let mapped = if flags == 0x0000_0100 {
            text.chars()
                .flat_map(char::to_lowercase)
                .collect::<String>()
        } else {
            text.chars()
                .flat_map(char::to_uppercase)
                .collect::<String>()
        };
        let output = GuestAddress(context.argument_u32(4)?);
        let capacity = context.argument_u32(5)?;
        if self.wide {
            let units = mapped.encode_utf16().collect::<Vec<_>>();
            let required = u32::try_from(units.len()).map_err(|_| Win32Error::OutOfMemory)?;
            if capacity == 0 {
                context.set_return_u32(required);
            } else if capacity < required || output.0 == 0 {
                context.set_last_error(122);
                context.set_return_u32(0);
            } else {
                let bytes = units
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>();
                context.write_memory(output, &bytes)?;
                context.set_return_u32(required);
            }
            context.set_stdcall_cleanup(24);
            Ok(())
        } else {
            let encoded = SHIFT_JIS.encode(&mapped).0;
            finish_conversion_bytes(context, output, capacity, &encoded, 24)
        }
    }
}

impl HostCallHandler for GetStringType {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let argument_base = u32::from(!self.wide);
        if context.argument_u32(argument_base as usize)? != 1 {
            return Err(Win32Error::Unsupported {
                feature: "GetStringType levels other than CT_CTYPE1",
            });
        }
        let input = GuestAddress(context.argument_u32(argument_base as usize + 1)?);
        let length = context.argument_u32(argument_base as usize + 2)? as i32;
        let units = if self.wide {
            read_guest_utf16_units(context, input, length)?
        } else {
            let (bytes, include_nul) = if length < 0 {
                (read_guest_z_bytes(context, input)?, true)
            } else if length > 0 {
                (
                    read_guest_bytes(
                        context,
                        input,
                        usize::try_from(length)
                            .map_err(|_| Win32Error::InvalidArgument("ANSI type length"))?,
                    )?,
                    false,
                )
            } else {
                return Err(Win32Error::InvalidArgument("ANSI type length"));
            };
            let (decoded, _) = SHIFT_JIS.decode_without_bom_handling(&bytes);
            let mut units = decoded.encode_utf16().collect::<Vec<_>>();
            if include_nul {
                units.push(0);
            }
            units
        };
        let types = units
            .into_iter()
            .flat_map(|unit| classify_ctype1(unit).to_le_bytes())
            .collect::<Vec<_>>();
        context.write_memory(
            GuestAddress(context.argument_u32(argument_base as usize + 3)?),
            &types,
        )?;
        context.set_last_error(0);
        context.set_return_u32(1);
        context.set_stdcall_cleanup(if self.wide { 16 } else { 20 });
        Ok(())
    }
}

impl HostCallHandler for CriticalSection {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        const STRUCTURE_SIZE: usize = 24;
        const LOCK_COUNT_OFFSET: usize = 4;
        const RECURSION_COUNT_OFFSET: usize = 8;
        const OWNER_OFFSET: usize = 12;
        let address = GuestAddress(context.argument_u32(0)?);
        let mut bytes = [0; STRUCTURE_SIZE];
        match self.operation {
            CriticalSectionOperation::Initialize
            | CriticalSectionOperation::InitializeAndSpinCount => {
                put_u32(&mut bytes, LOCK_COUNT_OFFSET, u32::MAX)?;
                context.write_memory(address, &bytes)?;
                if matches!(
                    self.operation,
                    CriticalSectionOperation::InitializeAndSpinCount
                ) {
                    let _spin_count = context.argument_u32(1)?;
                    context.set_return_u32(1);
                }
            }
            CriticalSectionOperation::Delete => {
                context.read_memory(address, &mut bytes)?;
                let recursion = get_u32(&bytes, RECURSION_COUNT_OFFSET)?;
                if recursion != 0 {
                    return Err(Win32Error::InvalidArgument(
                        "DeleteCriticalSection while owned",
                    ));
                }
                context.write_memory(address, &[0; STRUCTURE_SIZE])?;
            }
            CriticalSectionOperation::Enter => {
                context.read_memory(address, &mut bytes)?;
                let recursion = get_u32(&bytes, RECURSION_COUNT_OFFSET)?;
                let owner = get_u32(&bytes, OWNER_OFFSET)?;
                let thread = context.current_thread_id();
                if recursion != 0 && owner != thread {
                    return Err(Win32Error::Unsupported {
                        feature: "contended critical section scheduling",
                    });
                }
                let recursion = recursion.checked_add(1).ok_or(Win32Error::OutOfMemory)?;
                put_u32(&mut bytes, LOCK_COUNT_OFFSET, recursion - 1)?;
                put_u32(&mut bytes, RECURSION_COUNT_OFFSET, recursion)?;
                put_u32(&mut bytes, OWNER_OFFSET, thread)?;
                context.write_memory(address, &bytes)?;
            }
            CriticalSectionOperation::Leave => {
                context.read_memory(address, &mut bytes)?;
                let recursion = get_u32(&bytes, RECURSION_COUNT_OFFSET)?;
                if recursion == 0 || get_u32(&bytes, OWNER_OFFSET)? != context.current_thread_id() {
                    return Err(Win32Error::InvalidArgument(
                        "LeaveCriticalSection by non-owner",
                    ));
                }
                let recursion = recursion - 1;
                put_u32(
                    &mut bytes,
                    LOCK_COUNT_OFFSET,
                    if recursion == 0 {
                        u32::MAX
                    } else {
                        recursion - 1
                    },
                )?;
                put_u32(&mut bytes, RECURSION_COUNT_OFFSET, recursion)?;
                if recursion == 0 {
                    put_u32(&mut bytes, OWNER_OFFSET, 0)?;
                }
                context.write_memory(address, &bytes)?;
            }
        }
        context.set_stdcall_cleanup(
            if matches!(
                self.operation,
                CriticalSectionOperation::InitializeAndSpinCount
            ) {
                8
            } else {
                4
            },
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetModuleHandle {
    wide: bool,
}

impl HostCallHandler for GetModuleHandle {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let name_address = GuestAddress(context.argument_u32(0)?);
        if name_address.0 == 0 {
            context.set_last_error(0);
            context.set_return_u32(context.main_module_base().0);
        } else {
            let name = if self.wide {
                read_utf16_z(context, name_address)?
            } else {
                read_ansi_z(context, name_address)?
            };
            if let Some(handle) = context.loaded_module_handle(&name) {
                context.set_last_error(0);
                context.set_return_u32(handle.0);
            } else {
                // Win32 reports a missing module through the return value and
                // thread-local last error; it is not an exceptional Host-call
                // failure and callers commonly probe optional DLLs this way.
                context.set_last_error(126); // ERROR_MOD_NOT_FOUND
                context.set_return_u32(0);
            }
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetProcAddress;

#[derive(Debug, Clone, Copy)]
struct LoadLibrary {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct PointerCodec;

#[derive(Debug, Clone, Copy)]
enum InterlockedOperation {
    Increment,
    Decrement,
    Exchange,
    CompareExchange,
    ExchangeAdd,
}

#[derive(Debug, Clone, Copy)]
struct Interlocked {
    operation: InterlockedOperation,
}

impl HostCallHandler for PointerCodec {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        // A stable per-runtime cookie is sufficient for the observable Win32
        // contract here: encoded pointers are opaque and DecodePointer must
        // invert EncodePointer. XOR also maps null to a non-null token, as the
        // native API does.
        const POINTER_COOKIE: u32 = 0xA5C3_1F27;
        context.set_return_u32(context.argument_u32(0)? ^ POINTER_COOKIE);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for Interlocked {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let target = GuestAddress(context.argument_u32(0)?);
        let old = read_context_u32(context, target)?;
        let (new, result, cleanup) = match self.operation {
            InterlockedOperation::Increment => {
                let new = old.wrapping_add(1);
                (Some(new), new, 4)
            }
            InterlockedOperation::Decrement => {
                let new = old.wrapping_sub(1);
                (Some(new), new, 4)
            }
            InterlockedOperation::Exchange => (Some(context.argument_u32(1)?), old, 8),
            InterlockedOperation::CompareExchange => {
                let exchange = context.argument_u32(1)?;
                let comparand = context.argument_u32(2)?;
                ((old == comparand).then_some(exchange), old, 12)
            }
            InterlockedOperation::ExchangeAdd => {
                (Some(old.wrapping_add(context.argument_u32(1)?)), old, 8)
            }
        };
        if let Some(new) = new {
            context.write_memory(target, &new.to_le_bytes())?;
        }
        context.set_return_u32(result);
        context.set_stdcall_cleanup(cleanup);
        Ok(())
    }
}

impl HostCallHandler for LoadLibrary {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let name_address = GuestAddress(context.argument_u32(0)?);
        if name_address.0 == 0 {
            context.set_last_error(87); // ERROR_INVALID_PARAMETER
            context.set_return_u32(0);
            context.set_stdcall_cleanup(4);
            return Ok(());
        }
        let name = if self.wide {
            read_utf16_z(context, name_address)?
        } else {
            read_ansi_z(context, name_address)?
        };
        if let Some(handle) = context.loaded_module_handle(&name) {
            debug!(name, handle = handle.0, "loaded modeled Guest DLL");
            context.set_last_error(0);
            context.set_return_u32(handle.0);
        } else {
            debug!(name, "Guest DLL is not modeled");
            context.set_last_error(126); // ERROR_MOD_NOT_FOUND
            context.set_return_u32(0);
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for GetProcAddress {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let module = GuestAddress(context.argument_u32(0)?);
        let name_address = GuestAddress(context.argument_u32(1)?);
        if name_address.0 <= u32::from(u16::MAX) {
            return Err(Win32Error::Unsupported {
                feature: "GetProcAddress by ordinal",
            });
        }
        let name = read_ansi_z(context, name_address)?;
        match context.resolve_host_api(module, &name) {
            Ok(address) => {
                context.set_last_error(0);
                context.set_return_u32(address.0);
            }
            Err(Win32Error::ProcedureNotFound { .. }) => {
                context.set_last_error(127); // ERROR_PROC_NOT_FOUND
                context.set_return_u32(0);
            }
            Err(Win32Error::ModuleNotFound(_)) => {
                context.set_last_error(126); // ERROR_MOD_NOT_FOUND
                context.set_return_u32(0);
            }
            Err(error) => return Err(error),
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetCommandLine {
    wide: bool,
}

impl HostCallHandler for GetCommandLine {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let address = if self.wide {
            context.command_line_utf16()
        } else {
            context.command_line_ansi()
        };
        context.set_return_u32(address.0);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetModuleFileName {
    wide: bool,
}

impl HostCallHandler for GetModuleFileName {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let module = GuestAddress(context.argument_u32(0)?);
        let output = GuestAddress(context.argument_u32(1)?);
        let capacity = context.argument_u32(2)?;
        if module.0 != 0 && module != context.main_module_base() {
            context.set_last_error(126); // ERROR_MOD_NOT_FOUND
            context.set_return_u32(0);
            context.set_stdcall_cleanup(12);
            return Ok(());
        }
        if output.0 == 0 || capacity == 0 {
            context.set_last_error(122); // ERROR_INSUFFICIENT_BUFFER
            context.set_return_u32(0);
            context.set_stdcall_cleanup(12);
            return Ok(());
        }

        let path = context.main_module_path().to_owned();
        let result = if self.wide {
            write_module_path_utf16(context, output, capacity, &path)?
        } else {
            write_module_path_ansi(context, output, capacity, &path)?
        };
        context.set_return_u32(result);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetLastError;

impl HostCallHandler for GetLastError {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.set_return_u32(context.last_error());
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SetLastError;

impl HostCallHandler for SetLastError {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.set_last_error(context.argument_u32(0)?);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

fn write_module_path_ansi(
    context: &mut dyn HostCallContext,
    output: GuestAddress,
    capacity: u32,
    path: &str,
) -> Result<u32, Win32Error> {
    let encoded = encode_ansi_z(path);
    let capacity = usize::try_from(capacity)
        .map_err(|_| Win32Error::InvalidArgument("ANSI path capacity overflow"))?;
    if encoded.len() <= capacity {
        context.write_memory(output, &encoded)?;
        context.set_last_error(0);
        return u32::try_from(encoded.len() - 1)
            .map_err(|_| Win32Error::InvalidArgument("ANSI path length overflow"));
    }
    let mut truncated = encoded[..capacity].to_vec();
    if let Some(last) = truncated.last_mut() {
        *last = 0;
    }
    context.write_memory(output, &truncated)?;
    context.set_last_error(122);
    u32::try_from(capacity).map_err(|_| Win32Error::InvalidArgument("ANSI capacity overflow"))
}

fn write_module_path_utf16(
    context: &mut dyn HostCallContext,
    output: GuestAddress,
    capacity: u32,
    path: &str,
) -> Result<u32, Win32Error> {
    let encoded = encode_utf16_z(path);
    let capacity_bytes = usize::try_from(capacity)
        .ok()
        .and_then(|value| value.checked_mul(2))
        .ok_or(Win32Error::InvalidArgument("UTF-16 path capacity overflow"))?;
    if encoded.len() <= capacity_bytes {
        context.write_memory(output, &encoded)?;
        context.set_last_error(0);
        return u32::try_from(encoded.len() / 2 - 1)
            .map_err(|_| Win32Error::InvalidArgument("UTF-16 path length overflow"));
    }
    let mut truncated = encoded[..capacity_bytes].to_vec();
    if truncated.len() >= 2 {
        let end = truncated.len();
        truncated[end - 2..].fill(0);
    }
    context.write_memory(output, &truncated)?;
    context.set_last_error(122);
    Ok(capacity)
}

#[derive(Debug, Clone, Copy)]
struct CreateFile {
    wide: bool,
}

impl HostCallHandler for CreateFile {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let path_address = GuestAddress(context.argument_u32(0)?);
        let path = if self.wide {
            read_utf16_z(context, path_address)?
        } else {
            read_ansi_z(context, path_address)?
        };
        let desired_access = context.argument_u32(1)?;
        if desired_access & GENERIC_READ == 0 || desired_access & 0x4000_0000 != 0 {
            return Err(Win32Error::Unsupported {
                feature: "CreateFile access other than read-only",
            });
        }
        let security = GuestAddress(context.argument_u32(3)?);
        if security.0 != 0 {
            let mut attributes = [0; 12];
            context.read_memory(security, &mut attributes)?;
            if u32::from_le_bytes(
                attributes[..4].try_into().map_err(|_| {
                    Win32Error::InvalidArgument("CreateFile SECURITY_ATTRIBUTES size")
                })?,
            ) != 12
            {
                return Err(Win32Error::InvalidArgument(
                    "CreateFile SECURITY_ATTRIBUTES length",
                ));
            }
        }
        if context.argument_u32(4)? != OPEN_EXISTING || context.argument_u32(6)? != 0 {
            return Err(Win32Error::Unsupported {
                feature: "CreateFile creation or template mode",
            });
        }
        let flags = context.argument_u32(5)?;
        debug!(path, desired_access, flags, "Guest file open request");
        if flags & !(0x0000_0080 | 0x0200_0000) != 0 {
            return Err(Win32Error::Unsupported {
                feature: "CreateFile flags",
            });
        }
        match context.open_file_read(&path) {
            Ok(handle) => {
                debug!(path, handle = handle.0, "opened Guest file");
                context.set_last_error(0);
                context.set_return_u32(handle.0);
            }
            Err(_) => {
                debug!(path, "Guest file was not found");
                context.set_last_error(2); // ERROR_FILE_NOT_FOUND
                context.set_return_u32(INVALID_HANDLE_VALUE);
            }
        }
        context.set_stdcall_cleanup(28);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct ReadFile;

impl HostCallHandler for ReadFile {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        let output = GuestAddress(context.argument_u32(1)?);
        let length = usize::try_from(context.argument_u32(2)?)
            .map_err(|_| Win32Error::InvalidArgument("ReadFile length overflow"))?;
        let bytes_read = GuestAddress(context.argument_u32(3)?);
        if context.argument_u32(4)? != 0 {
            return Err(Win32Error::Unsupported {
                feature: "overlapped ReadFile",
            });
        }
        let bytes = match context.read_file(handle, length) {
            Ok(bytes) => bytes,
            Err(_) => {
                context.set_last_error(6); // ERROR_INVALID_HANDLE
                context.set_return_u32(0);
                context.set_stdcall_cleanup(20);
                return Ok(());
            }
        };
        context.write_memory(output, &bytes)?;
        if bytes_read.0 != 0 {
            let count = u32::try_from(bytes.len())
                .map_err(|_| Win32Error::InvalidArgument("ReadFile result overflow"))?;
            context.write_memory(bytes_read, &count.to_le_bytes())?;
        }
        context.set_last_error(0);
        context.set_return_u32(1);
        context.set_stdcall_cleanup(20);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct FindFirstFileA;

impl HostCallHandler for FindFirstFileA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let pattern = read_ansi_z(context, GuestAddress(context.argument_u32(0)?))?;
        let output = GuestAddress(context.argument_u32(1)?);
        match context.find_first_file(&pattern) {
            Ok((handle, entry)) => {
                write_find_data_a(context, output, &entry)?;
                context.set_last_error(0);
                context.set_return_u32(handle.0);
            }
            Err(_) => {
                context.set_last_error(2);
                context.set_return_u32(INVALID_HANDLE_VALUE);
            }
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct FindNextFileA;

impl HostCallHandler for FindNextFileA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        let output = GuestAddress(context.argument_u32(1)?);
        match context.find_next_file(handle) {
            Ok(Some(entry)) => {
                write_find_data_a(context, output, &entry)?;
                context.set_last_error(0);
                context.set_return_u32(1);
            }
            Ok(None) => {
                context.set_last_error(18); // ERROR_NO_MORE_FILES
                context.set_return_u32(0);
            }
            Err(_) => {
                context.set_last_error(6);
                context.set_return_u32(0);
            }
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct FindClose;

impl HostCallHandler for FindClose {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let success = context
            .close_file_search(Handle(context.argument_u32(0)?))
            .is_ok();
        context.set_last_error(if success { 0 } else { 6 });
        context.set_return_u32(u32::from(success));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum ResourceIdentifier {
    Id(u16),
    Name(String),
}

#[derive(Debug, Clone, Copy)]
struct FindResourceA;

impl HostCallHandler for FindResourceA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let module = GuestAddress(context.argument_u32(0)?);
        if module.0 != 0 && module != context.main_module_base() {
            context.set_last_error(126);
            context.set_return_u32(0);
            context.set_stdcall_cleanup(12);
            return Ok(());
        }
        let name = read_resource_identifier(context, context.argument_u32(1)?)?;
        let resource_type = read_resource_identifier(context, context.argument_u32(2)?)?;
        let Some((root, size)) = context.resource_directory() else {
            context.set_last_error(1812); // ERROR_RESOURCE_DATA_NOT_FOUND
            context.set_return_u32(0);
            context.set_stdcall_cleanup(12);
            return Ok(());
        };
        let resource = find_resource_data_entry(context, root, size, &resource_type, &name)?;
        context.set_last_error(if resource.is_some() { 0 } else { 1814 });
        context.set_return_u32(resource.map_or(0, |address| address.0));
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SizeofResource;

impl HostCallHandler for SizeofResource {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let module = GuestAddress(context.argument_u32(0)?);
        let resource = GuestAddress(context.argument_u32(1)?);
        if (module.0 != 0 && module != context.main_module_base())
            || !resource_data_entry_is_valid(context, resource)
        {
            context.set_last_error(1813);
            context.set_return_u32(0);
        } else {
            context.set_last_error(0);
            context.set_return_u32(read_context_u32(context, GuestAddress(resource.0 + 4))?);
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct LoadResource;

impl HostCallHandler for LoadResource {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let module = GuestAddress(context.argument_u32(0)?);
        let resource = GuestAddress(context.argument_u32(1)?);
        let valid = (module.0 == 0 || module == context.main_module_base())
            && resource_data_entry_is_valid(context, resource);
        context.set_last_error(if valid { 0 } else { 1813 });
        context.set_return_u32(if valid { resource.0 } else { 0 });
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct LockResource;

impl HostCallHandler for LockResource {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let resource = GuestAddress(context.argument_u32(0)?);
        if resource_data_entry_is_valid(context, resource) {
            let rva = read_context_u32(context, resource)?;
            let address = context
                .main_module_base()
                .0
                .checked_add(rva)
                .ok_or(Win32Error::InvalidArgument("resource RVA overflow"))?;
            context.set_return_u32(address);
        } else {
            context.set_return_u32(0);
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct FreeResource;

impl HostCallHandler for FreeResource {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _resource = context.argument_u32(0)?;
        context.set_return_u32(0); // Obsolete on Win32; resources remain mapped.
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct CloseHandle;

impl HostCallHandler for CloseHandle {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        if context.close_kernel_handle(handle).is_err() {
            context.set_last_error(6);
            context.set_return_u32(0);
        } else {
            context.set_last_error(0);
            context.set_return_u32(1);
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetStdHandle;

impl HostCallHandler for GetStdHandle {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let raw = context.argument_u32(0)?;
        let selector = i32::from_ne_bytes(raw.to_ne_bytes());
        if let Some(handle) = context.standard_handle(selector) {
            context.set_last_error(0);
            context.set_return_u32(handle.0);
        } else {
            context.set_last_error(87); // ERROR_INVALID_PARAMETER
            context.set_return_u32(INVALID_HANDLE_VALUE);
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct WriteFile;

impl HostCallHandler for WriteFile {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        let input = GuestAddress(context.argument_u32(1)?);
        let length = usize::try_from(context.argument_u32(2)?)
            .map_err(|_| Win32Error::InvalidArgument("WriteFile length overflow"))?;
        let bytes_written = GuestAddress(context.argument_u32(3)?);
        if context.argument_u32(4)? != 0 {
            return Err(Win32Error::Unsupported {
                feature: "overlapped WriteFile",
            });
        }
        let mut bytes = vec![0; length];
        context.read_memory(input, &mut bytes)?;
        let written = match context.write_handle(handle, &bytes) {
            Ok(written) => written,
            Err(_) => {
                context.set_last_error(6);
                context.set_return_u32(0);
                context.set_stdcall_cleanup(20);
                return Ok(());
            }
        };
        if bytes_written.0 != 0 {
            let written = u32::try_from(written)
                .map_err(|_| Win32Error::InvalidArgument("WriteFile result overflow"))?;
            context.write_memory(bytes_written, &written.to_le_bytes())?;
        }
        context.set_last_error(0);
        context.set_return_u32(1);
        context.set_stdcall_cleanup(20);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetFileSize;

impl HostCallHandler for GetFileSize {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        let high_output = GuestAddress(context.argument_u32(1)?);
        let size = match context.file_size(handle) {
            Ok(size) => size,
            Err(_) => {
                context.set_last_error(6);
                context.set_return_u32(u32::MAX);
                context.set_stdcall_cleanup(8);
                return Ok(());
            }
        };
        let bytes = size.to_le_bytes();
        let low = u32::from_le_bytes(
            bytes[..4]
                .try_into()
                .map_err(|_| Win32Error::InvalidArgument("GetFileSize low conversion failed"))?,
        );
        let high = u32::from_le_bytes(
            bytes[4..]
                .try_into()
                .map_err(|_| Win32Error::InvalidArgument("GetFileSize high conversion failed"))?,
        );
        if high_output.0 != 0 {
            context.write_memory(high_output, &high.to_le_bytes())?;
        }
        context.set_last_error(0);
        context.set_return_u32(low);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SetFilePointer;

impl HostCallHandler for SetFilePointer {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        let low_raw = context.argument_u32(1)?;
        let high_address = GuestAddress(context.argument_u32(2)?);
        let origin = context.argument_u32(3)?;
        let distance = if high_address.0 == 0 {
            i64::from(i32::from_ne_bytes(low_raw.to_ne_bytes()))
        } else {
            let mut high_bytes = [0; 4];
            context.read_memory(high_address, &mut high_bytes)?;
            let high = i32::from_le_bytes(high_bytes);
            (i64::from(high) << 32) | i64::from(low_raw)
        };
        let position = match context.seek_file(handle, distance, origin) {
            Ok(position) => position,
            Err(_) => {
                context.set_last_error(87);
                context.set_return_u32(u32::MAX);
                context.set_stdcall_cleanup(16);
                return Ok(());
            }
        };
        let bytes = position.to_le_bytes();
        let low =
            u32::from_le_bytes(bytes[..4].try_into().map_err(|_| {
                Win32Error::InvalidArgument("SetFilePointer low conversion failed")
            })?);
        if high_address.0 != 0 {
            context.write_memory(high_address, &bytes[4..])?;
        }
        context.set_last_error(0);
        context.set_return_u32(low);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetFileType;

impl HostCallHandler for GetFileType {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        if let Some(file_type) = context.file_type(handle) {
            context.set_last_error(0);
            context.set_return_u32(file_type);
        } else {
            context.set_last_error(6);
            context.set_return_u32(0);
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetEnvironmentVariable {
    wide: bool,
}

impl HostCallHandler for GetEnvironmentVariable {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let name_address = GuestAddress(context.argument_u32(0)?);
        let output = GuestAddress(context.argument_u32(1)?);
        let capacity = context.argument_u32(2)?;
        let name = if self.wide {
            read_utf16_z(context, name_address)?
        } else {
            read_ansi_z(context, name_address)?
        };
        let Some(value) = context.environment_variable(&name).map(str::to_owned) else {
            context.set_last_error(203); // ERROR_ENVVAR_NOT_FOUND
            context.set_return_u32(0);
            context.set_stdcall_cleanup(12);
            return Ok(());
        };
        let (encoded, required_units) = if self.wide {
            let bytes = encode_utf16_z(&value);
            let units = u32::try_from(bytes.len() / 2)
                .map_err(|_| Win32Error::InvalidArgument("environment value too long"))?;
            (bytes, units)
        } else {
            let bytes = encode_ansi_z(&value);
            let units = u32::try_from(bytes.len())
                .map_err(|_| Win32Error::InvalidArgument("environment value too long"))?;
            (bytes, units)
        };
        if output.0 == 0 || capacity < required_units {
            context.set_last_error(122);
            context.set_return_u32(required_units);
            context.set_stdcall_cleanup(12);
            return Ok(());
        }
        context.write_memory(output, &encoded)?;
        context.set_last_error(0);
        context.set_return_u32(required_units - 1);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetEnvironmentStrings {
    wide: bool,
}

impl HostCallHandler for GetEnvironmentStrings {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let address = if self.wide {
            context.environment_block_utf16()
        } else {
            context.environment_block_ansi()
        };
        context.set_return_u32(address.0);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct FreeEnvironmentStrings {
    wide: bool,
}

impl HostCallHandler for FreeEnvironmentStrings {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let address = GuestAddress(context.argument_u32(0)?);
        let expected = if self.wide {
            context.environment_block_utf16()
        } else {
            context.environment_block_ansi()
        };
        if address == expected {
            context.set_last_error(0);
            context.set_return_u32(1);
        } else {
            context.set_last_error(87); // ERROR_INVALID_PARAMETER
            context.set_return_u32(0);
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetCurrentDirectory {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct SetCurrentDirectoryA;

impl HostCallHandler for SetCurrentDirectoryA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let path = read_ansi_z(context, GuestAddress(context.argument_u32(0)?))?;
        let success = context.set_current_directory(&path).is_ok();
        context.set_last_error(if success { 0 } else { 3 });
        context.set_return_u32(u32::from(success));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for GetCurrentDirectory {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let capacity = context.argument_u32(0)?;
        let output = GuestAddress(context.argument_u32(1)?);
        let current_directory = context.current_directory().to_owned();
        let (encoded, required_units) = if self.wide {
            let bytes = encode_utf16_z(&current_directory);
            let units = u32::try_from(bytes.len() / 2)
                .map_err(|_| Win32Error::InvalidArgument("current directory too long"))?;
            (bytes, units)
        } else {
            let bytes = encode_ansi_z(&current_directory);
            let units = u32::try_from(bytes.len())
                .map_err(|_| Win32Error::InvalidArgument("current directory too long"))?;
            (bytes, units)
        };
        if output.0 == 0 || capacity < required_units {
            context.set_last_error(122); // ERROR_INSUFFICIENT_BUFFER
            context.set_return_u32(required_units);
            context.set_stdcall_cleanup(8);
            return Ok(());
        }
        context.write_memory(output, &encoded)?;
        context.set_last_error(0);
        context.set_return_u32(required_units - 1);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct CurrentId {
    thread: bool,
}

#[derive(Debug, Clone, Copy)]
struct CurrentPseudoHandle {
    thread: bool,
}

#[derive(Debug, Clone, Copy)]
struct GetThreadPreferredUiLanguages;

#[derive(Debug, Clone, Copy)]
struct GetFullPathName {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct GetFileTime;

#[derive(Debug, Clone, Copy)]
struct OutputDebugString {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct GetShortPathName {
    wide: bool,
}

impl HostCallHandler for CurrentId {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let value = if self.thread {
            context.current_thread_id()
        } else {
            context.current_process_id()
        };
        context.set_return_u32(value);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

impl HostCallHandler for CurrentPseudoHandle {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.set_return_u32(if self.thread { u32::MAX - 1 } else { u32::MAX });
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

impl HostCallHandler for GetThreadPreferredUiLanguages {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let language = match context.argument_u32(0)? {
            0x0000_0004 => "0804",  // MUI_LANGUAGE_ID
            0x0000_0008 => "zh-CN", // MUI_LANGUAGE_NAME
            _ => {
                return Err(Win32Error::Unsupported {
                    feature: "GetThreadPreferredUILanguages flags",
                });
            }
        };
        let mut languages = encode_utf16_z(language);
        languages.extend_from_slice(&[0, 0]);
        let required_units =
            u32::try_from(languages.len() / 2).map_err(|_| Win32Error::OutOfMemory)?;
        let count_output = GuestAddress(context.argument_u32(1)?);
        let buffer = GuestAddress(context.argument_u32(2)?);
        let size_pointer = GuestAddress(context.argument_u32(3)?);
        let supplied_units = read_context_u32(context, size_pointer)?;
        context.write_memory(count_output, &1_u32.to_le_bytes())?;
        context.write_memory(size_pointer, &required_units.to_le_bytes())?;
        if buffer.0 == 0 && supplied_units == 0 {
            context.set_last_error(0);
            context.set_return_u32(1);
        } else if supplied_units < required_units {
            context.set_last_error(122); // ERROR_INSUFFICIENT_BUFFER
            context.set_return_u32(0);
        } else {
            context.write_memory(buffer, &languages)?;
            context.set_last_error(0);
            context.set_return_u32(1);
        }
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

impl HostCallHandler for GetFullPathName {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let source = GuestAddress(context.argument_u32(0)?);
        let path = if self.wide {
            read_utf16_z(context, source)?
        } else {
            read_ansi_z(context, source)?
        };
        let absolute = absolute_windows_path(context.current_directory(), &path)?;
        let buffer_units = context.argument_u32(1)?;
        let buffer = GuestAddress(context.argument_u32(2)?);
        let file_part_output = GuestAddress(context.argument_u32(3)?);
        let (encoded, length_without_nul, file_part_offset) = if self.wide {
            let encoded = encode_utf16_z(&absolute);
            let length =
                u32::try_from(encoded.len() / 2 - 1).map_err(|_| Win32Error::OutOfMemory)?;
            let file_part = absolute
                .rfind('\\')
                .map_or(0, |index| absolute[..=index].encode_utf16().count());
            (
                encoded,
                length,
                u32::try_from(file_part).map_err(|_| Win32Error::OutOfMemory)?,
            )
        } else {
            let encoded = encode_ansi_z(&absolute);
            let length = u32::try_from(encoded.len() - 1).map_err(|_| Win32Error::OutOfMemory)?;
            let file_part = encoded[..encoded.len() - 1]
                .iter()
                .rposition(|byte| *byte == b'\\')
                .map_or(0, |index| index + 1);
            (
                encoded,
                length,
                u32::try_from(file_part).map_err(|_| Win32Error::OutOfMemory)?,
            )
        };
        let required_units = length_without_nul
            .checked_add(1)
            .ok_or(Win32Error::OutOfMemory)?;
        if buffer_units < required_units {
            context.set_return_u32(required_units);
        } else {
            context.write_memory(buffer, &encoded)?;
            if file_part_output.0 != 0 {
                let stride = if self.wide { 2 } else { 1 };
                let file_part = buffer
                    .0
                    .checked_add(
                        file_part_offset
                            .checked_mul(stride)
                            .ok_or(Win32Error::OutOfMemory)?,
                    )
                    .ok_or(Win32Error::OutOfMemory)?;
                context.write_memory(file_part_output, &file_part.to_le_bytes())?;
            }
            context.set_return_u32(length_without_nul);
        }
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

impl HostCallHandler for GetFileTime {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        if context.file_size(handle).is_err() {
            context.set_last_error(6); // ERROR_INVALID_HANDLE
            context.set_return_u32(0);
            context.set_stdcall_cleanup(16);
            return Ok(());
        }
        let time = context.system_time_filetime().to_le_bytes();
        for index in 1..=3 {
            let output = GuestAddress(context.argument_u32(index)?);
            if output.0 != 0 {
                context.write_memory(output, &time)?;
            }
        }
        context.set_last_error(0);
        context.set_return_u32(1);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

impl HostCallHandler for OutputDebugString {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let address = GuestAddress(context.argument_u32(0)?);
        let message = if self.wide {
            read_utf16_z(context, address)?
        } else {
            read_ansi_z(context, address)?
        };
        debug!(message, "Guest debug string");
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for GetShortPathName {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let source = GuestAddress(context.argument_u32(0)?);
        let path = if self.wide {
            read_utf16_z(context, source)?
        } else {
            read_ansi_z(context, source)?
        };
        // The sandbox does not assign DOS 8.3 aliases. Windows permits volumes
        // with short-name generation disabled, so preserve the canonical path.
        let encoded = if self.wide {
            encode_utf16_z(&path)
        } else {
            encode_ansi_z(&path)
        };
        let stride = if self.wide { 2 } else { 1 };
        let required_units =
            u32::try_from(encoded.len() / stride).map_err(|_| Win32Error::OutOfMemory)?;
        let output = GuestAddress(context.argument_u32(1)?);
        let supplied_units = context.argument_u32(2)?;
        if output.0 == 0 || supplied_units < required_units {
            context.set_return_u32(required_units);
        } else {
            context.write_memory(output, &encoded)?;
            context.set_return_u32(required_units - 1);
        }
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

fn absolute_windows_path(current_directory: &str, path: &str) -> Result<String, Win32Error> {
    let path = path.replace('/', "\\");
    let current = current_directory.replace('/', "\\");
    let current_drive = current
        .get(..2)
        .filter(|prefix| prefix.ends_with(':'))
        .unwrap_or("C:");
    let (drive, tail) = if path.as_bytes().get(1) == Some(&b':') {
        (&path[..2], &path[2..])
    } else if path.starts_with('\\') {
        (current_drive, path.as_str())
    } else {
        let combined = format!("{}\\{}", current.trim_end_matches('\\'), path);
        return absolute_windows_path(current_drive, &combined);
    };
    let mut components = Vec::new();
    for component in tail.split('\\') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            value => components.push(value),
        }
    }
    Ok(format!("{drive}\\{}", components.join("\\")))
}

#[derive(Debug, Clone, Copy)]
struct VirtualAlloc;

#[derive(Debug, Clone, Copy)]
struct VirtualProtect;

impl HostCallHandler for VirtualProtect {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let address = GuestAddress(context.argument_u32(0)?);
        let size = context.argument_u32(1)?;
        let (read, write, execute) = virtual_protection(context.argument_u32(2)?)?;
        let old = context.protect_virtual_memory(address, size, read, write, execute)?;
        let old_protection = protection_flags(old)?;
        context.write_memory(
            GuestAddress(context.argument_u32(3)?),
            &old_protection.to_le_bytes(),
        )?;
        context.set_last_error(0);
        context.set_return_u32(1);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

impl HostCallHandler for VirtualAlloc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let requested_base = GuestAddress(context.argument_u32(0)?);
        let size = context.argument_u32(1)?;
        if size == 0 {
            return Err(Win32Error::InvalidArgument("VirtualAlloc zero size"));
        }
        let (read, write, execute) = virtual_protection(context.argument_u32(3)?)?;
        let address = match context.argument_u32(2)? {
            MEM_RESERVE if requested_base.0 == 0 => context.reserve_virtual_memory(size)?,
            MEM_COMMIT if requested_base.0 != 0 => {
                context.commit_virtual_memory(requested_base, size, read, write, execute)?;
                requested_base
            }
            MEM_COMMIT if requested_base.0 == 0 => {
                context.allocate_virtual_memory(size, read, write, execute)?
            }
            MEM_COMMIT_RESERVE if requested_base.0 == 0 => {
                context.allocate_virtual_memory(size, read, write, execute)?
            }
            MEM_RESERVE | MEM_COMMIT | MEM_COMMIT_RESERVE => {
                return Err(Win32Error::Unsupported {
                    feature: "VirtualAlloc base/allocation-type combination",
                });
            }
            _ => {
                return Err(Win32Error::Unsupported {
                    feature: "VirtualAlloc allocation type",
                });
            }
        };
        context.set_return_u32(address.0);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct VirtualFree;

impl HostCallHandler for VirtualFree {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let address = GuestAddress(context.argument_u32(0)?);
        if context.argument_u32(1)? != 0 || context.argument_u32(2)? != MEM_RELEASE {
            return Err(Win32Error::Unsupported {
                feature: "VirtualFree mode other than size=0, MEM_RELEASE",
            });
        }
        context.free_virtual_memory(address)?;
        context.set_return_u32(1);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

fn virtual_protection(protection: u32) -> Result<(bool, bool, bool), Win32Error> {
    match protection {
        0x01 => Ok((false, false, false)), // PAGE_NOACCESS
        0x02 => Ok((true, false, false)),  // PAGE_READONLY
        0x04 => Ok((true, true, false)),   // PAGE_READWRITE
        0x10 => Ok((false, false, true)),  // PAGE_EXECUTE
        0x20 => Ok((true, false, true)),   // PAGE_EXECUTE_READ
        0x40 => Ok((true, true, true)),    // PAGE_EXECUTE_READWRITE
        _ => Err(Win32Error::Unsupported {
            feature: "VirtualAlloc page protection flags",
        }),
    }
}

fn protection_flags(permissions: (bool, bool, bool)) -> Result<u32, Win32Error> {
    match permissions {
        (false, false, false) => Ok(0x01),
        (true, false, false) => Ok(0x02),
        (true, true, false) => Ok(0x04),
        (false, false, true) => Ok(0x10),
        (true, false, true) => Ok(0x20),
        (true, true, true) => Ok(0x40),
        _ => Err(Win32Error::Unsupported {
            feature: "VirtualProtect permission combination",
        }),
    }
}

fn read_resource_identifier(
    context: &dyn HostCallContext,
    raw: u32,
) -> Result<ResourceIdentifier, Win32Error> {
    if raw <= u32::from(u16::MAX) {
        Ok(ResourceIdentifier::Id(raw as u16))
    } else {
        Ok(ResourceIdentifier::Name(read_ansi_z(
            context,
            GuestAddress(raw),
        )?))
    }
}

fn find_resource_data_entry(
    context: &dyn HostCallContext,
    root: GuestAddress,
    size: u32,
    resource_type: &ResourceIdentifier,
    name: &ResourceIdentifier,
) -> Result<Option<GuestAddress>, Win32Error> {
    let Some(type_entry) = find_resource_directory_entry(context, root, size, root, resource_type)?
    else {
        return Ok(None);
    };
    let Some(type_directory) = resource_subdirectory(root, size, type_entry)? else {
        return Ok(None);
    };
    let Some(name_entry) =
        find_resource_directory_entry(context, root, size, type_directory, name)?
    else {
        return Ok(None);
    };
    let Some(language_directory) = resource_subdirectory(root, size, name_entry)? else {
        return Ok(None);
    };
    let mut header = [0; 16];
    context.read_memory(language_directory, &mut header)?;
    let entry_count = u32::from(u16::from_le_bytes([header[12], header[13]]))
        + u32::from(u16::from_le_bytes([header[14], header[15]]));
    if entry_count == 0 {
        return Ok(None);
    }
    let first_entry = resource_offset_address(root, size, language_directory.0 - root.0 + 16, 8)?;
    let data_offset = read_context_u32(context, GuestAddress(first_entry.0 + 4))?;
    if data_offset & 0x8000_0000 != 0 {
        return Err(Win32Error::InvalidArgument(
            "resource language entry points to directory",
        ));
    }
    resource_offset_address(root, size, data_offset, 16).map(Some)
}

fn find_resource_directory_entry(
    context: &dyn HostCallContext,
    root: GuestAddress,
    size: u32,
    directory: GuestAddress,
    identifier: &ResourceIdentifier,
) -> Result<Option<u32>, Win32Error> {
    let mut header = [0; 16];
    context.read_memory(directory, &mut header)?;
    let named = u32::from(u16::from_le_bytes([header[12], header[13]]));
    let ids = u32::from(u16::from_le_bytes([header[14], header[15]]));
    let total = named
        .checked_add(ids)
        .filter(|total| *total <= 4096)
        .ok_or(Win32Error::InvalidArgument(
            "resource directory entry count",
        ))?;
    for index in 0..total {
        let relative = directory
            .0
            .checked_sub(root.0)
            .and_then(|offset| offset.checked_add(16))
            .and_then(|offset| offset.checked_add(index * 8))
            .ok_or(Win32Error::InvalidArgument("resource entry offset"))?;
        let entry = resource_offset_address(root, size, relative, 8)?;
        let name_field = read_context_u32(context, entry)?;
        let matches = match identifier {
            ResourceIdentifier::Id(id) => {
                name_field & 0x8000_0000 == 0 && name_field & 0xffff == u32::from(*id)
            }
            ResourceIdentifier::Name(expected) => {
                if name_field & 0x8000_0000 == 0 {
                    false
                } else {
                    read_resource_directory_name(context, root, size, name_field & 0x7fff_ffff)?
                        == *expected
                }
            }
        };
        if matches {
            return read_context_u32(context, GuestAddress(entry.0 + 4)).map(Some);
        }
    }
    Ok(None)
}

fn resource_subdirectory(
    root: GuestAddress,
    size: u32,
    entry: u32,
) -> Result<Option<GuestAddress>, Win32Error> {
    if entry & 0x8000_0000 == 0 {
        return Ok(None);
    }
    resource_offset_address(root, size, entry & 0x7fff_ffff, 16).map(Some)
}

fn read_resource_directory_name(
    context: &dyn HostCallContext,
    root: GuestAddress,
    size: u32,
    offset: u32,
) -> Result<String, Win32Error> {
    let length_address = resource_offset_address(root, size, offset, 2)?;
    let mut length_bytes = [0; 2];
    context.read_memory(length_address, &mut length_bytes)?;
    let length = usize::from(u16::from_le_bytes(length_bytes));
    let byte_length = length
        .checked_mul(2)
        .ok_or(Win32Error::InvalidArgument("resource name length"))?;
    let string_address = resource_offset_address(root, size, offset + 2, byte_length as u32)?;
    let mut bytes = vec![0; byte_length];
    context.read_memory(string_address, &mut bytes)?;
    let units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| Win32Error::InvalidArgument("resource name UTF-16"))
}

fn resource_offset_address(
    root: GuestAddress,
    size: u32,
    offset: u32,
    length: u32,
) -> Result<GuestAddress, Win32Error> {
    offset
        .checked_add(length)
        .filter(|end| *end <= size)
        .and_then(|_| root.0.checked_add(offset))
        .map(GuestAddress)
        .ok_or(Win32Error::InvalidArgument("resource directory bounds"))
}

fn resource_data_entry_is_valid(context: &dyn HostCallContext, entry: GuestAddress) -> bool {
    context.resource_directory().is_some_and(|(root, size)| {
        entry.0 >= root.0
            && entry
                .0
                .checked_add(16)
                .is_some_and(|end| end <= root.0.saturating_add(size))
    })
}

fn read_context_u32(
    context: &dyn HostCallContext,
    address: GuestAddress,
) -> Result<u32, Win32Error> {
    let mut bytes = [0; 4];
    context.read_memory(address, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn windows_encoding(code_page: u32) -> Result<&'static Encoding, Win32Error> {
    match code_page {
        0 | 3 | 932 => Ok(SHIFT_JIS), // CP_ACP, CP_THREAD_ACP, Japanese Windows
        65_001 => Ok(UTF_8),
        _ => Err(Win32Error::Unsupported {
            feature: "requested Windows code page",
        }),
    }
}

fn write_find_data_a(
    context: &mut dyn HostCallContext,
    output: GuestAddress,
    entry: &FileEntry,
) -> Result<(), Win32Error> {
    let name = encode_ansi_z(&entry.name);
    if name.len() > 260 {
        return Err(Win32Error::InvalidArgument("WIN32_FIND_DATA filename"));
    }
    let mut data = [0; 320];
    put_u32(&mut data, 0, if entry.is_directory { 0x10 } else { 0x80 })?;
    put_u32(&mut data, 28, (entry.size >> 32) as u32)?;
    put_u32(&mut data, 32, entry.size as u32)?;
    data[44..44 + name.len()].copy_from_slice(&name);
    context.write_memory(output, &data)
}

fn read_guest_bytes(
    context: &dyn HostCallContext,
    address: GuestAddress,
    length: usize,
) -> Result<Vec<u8>, Win32Error> {
    if address.0 == 0 || length > MAX_GUEST_STRING_BYTES {
        return Err(Win32Error::InvalidArgument("guest conversion input"));
    }
    let mut bytes = vec![0; length];
    context.read_memory(address, &mut bytes)?;
    Ok(bytes)
}

fn read_guest_z_bytes(
    context: &dyn HostCallContext,
    address: GuestAddress,
) -> Result<Vec<u8>, Win32Error> {
    if address.0 == 0 {
        return Err(Win32Error::InvalidArgument("null conversion input"));
    }
    let mut bytes = Vec::new();
    for offset in 0..MAX_GUEST_STRING_BYTES {
        let offset = u32::try_from(offset)
            .map_err(|_| Win32Error::InvalidArgument("conversion input offset"))?;
        let current = address
            .0
            .checked_add(offset)
            .map(GuestAddress)
            .ok_or(Win32Error::InvalidArgument("conversion input overflow"))?;
        let mut byte = [0];
        context.read_memory(current, &mut byte)?;
        if byte[0] == 0 {
            return Ok(bytes);
        }
        bytes.push(byte[0]);
    }
    Err(Win32Error::InvalidArgument("unterminated conversion input"))
}

fn read_guest_utf16_units(
    context: &dyn HostCallContext,
    address: GuestAddress,
    length: i32,
) -> Result<Vec<u16>, Win32Error> {
    if address.0 == 0 || length == 0 {
        return Err(Win32Error::InvalidArgument("UTF-16 type input"));
    }
    if length > 0 {
        let count = usize::try_from(length)
            .map_err(|_| Win32Error::InvalidArgument("UTF-16 type length"))?;
        let bytes = read_guest_bytes(context, address, count.saturating_mul(2))?;
        return Ok(bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect());
    }
    let mut units = Vec::new();
    for index in 0..MAX_GUEST_STRING_BYTES / 2 {
        let offset = u32::try_from(index)
            .ok()
            .and_then(|index| index.checked_mul(2))
            .ok_or(Win32Error::InvalidArgument("UTF-16 type offset"))?;
        let current = address
            .0
            .checked_add(offset)
            .map(GuestAddress)
            .ok_or(Win32Error::InvalidArgument("UTF-16 type overflow"))?;
        let mut bytes = [0; 2];
        context.read_memory(current, &mut bytes)?;
        let unit = u16::from_le_bytes(bytes);
        units.push(unit);
        if unit == 0 {
            return Ok(units);
        }
    }
    Err(Win32Error::InvalidArgument(
        "unterminated UTF-16 type input",
    ))
}

fn classify_ctype1(unit: u16) -> u16 {
    const C1_UPPER: u16 = 0x0001;
    const C1_LOWER: u16 = 0x0002;
    const C1_DIGIT: u16 = 0x0004;
    const C1_SPACE: u16 = 0x0008;
    const C1_PUNCT: u16 = 0x0010;
    const C1_CNTRL: u16 = 0x0020;
    const C1_BLANK: u16 = 0x0040;
    const C1_XDIGIT: u16 = 0x0080;
    const C1_ALPHA: u16 = 0x0100;
    const C1_DEFINED: u16 = 0x0200;
    let Some(character) = char::from_u32(u32::from(unit)) else {
        return C1_DEFINED;
    };
    let mut kind = 0;
    if character.is_uppercase() {
        kind |= C1_UPPER;
    }
    if character.is_lowercase() {
        kind |= C1_LOWER;
    }
    if character.is_numeric() {
        kind |= C1_DIGIT;
    }
    if character.is_whitespace() {
        kind |= C1_SPACE;
    }
    if matches!(character, ' ' | '\t') {
        kind |= C1_BLANK;
    }
    if character.is_control() {
        kind |= C1_CNTRL;
    }
    if character.is_ascii_hexdigit() {
        kind |= C1_XDIGIT;
    }
    if character.is_alphabetic() {
        kind |= C1_ALPHA;
    }
    if character.is_ascii_punctuation() || matches!(unit, 0x3000..=0x303f | 0xff01..=0xff65) {
        kind |= C1_PUNCT;
    }
    if kind == 0 && unit != 0 {
        kind = C1_DEFINED;
    }
    kind
}

fn finish_conversion_bytes(
    context: &mut dyn HostCallContext,
    output: GuestAddress,
    capacity: u32,
    bytes: &[u8],
    cleanup: u32,
) -> Result<(), Win32Error> {
    let required = u32::try_from(bytes.len()).map_err(|_| Win32Error::OutOfMemory)?;
    if capacity == 0 {
        context.set_last_error(0);
        context.set_return_u32(required);
    } else if capacity < required || output.0 == 0 {
        context.set_last_error(122); // ERROR_INSUFFICIENT_BUFFER
        context.set_return_u32(0);
    } else {
        context.write_memory(output, bytes)?;
        context.set_last_error(0);
        context.set_return_u32(required);
    }
    context.set_stdcall_cleanup(cleanup);
    Ok(())
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), Win32Error> {
    let end = offset
        .checked_add(4)
        .ok_or(Win32Error::InvalidArgument("u32 field offset overflow"))?;
    let target = bytes
        .get_mut(offset..end)
        .ok_or(Win32Error::InvalidArgument("u32 field outside structure"))?;
    target.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) -> Result<(), Win32Error> {
    let end = offset
        .checked_add(2)
        .ok_or(Win32Error::InvalidArgument("u16 field offset overflow"))?;
    let target = bytes
        .get_mut(offset..end)
        .ok_or(Win32Error::InvalidArgument("u16 field outside structure"))?;
    target.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn get_u32(bytes: &[u8], offset: usize) -> Result<u32, Win32Error> {
    let end = offset
        .checked_add(4)
        .ok_or(Win32Error::InvalidArgument("u32 field offset overflow"))?;
    let source = bytes
        .get(offset..end)
        .ok_or(Win32Error::InvalidArgument("u32 field outside structure"))?;
    let mut value = [0; 4];
    value.copy_from_slice(source);
    Ok(u32::from_le_bytes(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_initial_surface() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        for name in [
            "ExitProcess",
            "SetUnhandledExceptionFilter",
            "UnhandledExceptionFilter",
            "LoadLibraryA",
            "LoadLibraryW",
            "EncodePointer",
            "DecodePointer",
            "InitializeCriticalSectionAndSpinCount",
            "InterlockedIncrement",
            "InterlockedDecrement",
            "InterlockedExchange",
            "InterlockedCompareExchange",
            "InterlockedExchangeAdd",
            "GetCurrentProcess",
            "GetCurrentThread",
            "GetThreadPreferredUILanguages",
            "GetFullPathNameA",
            "GetFullPathNameW",
            "GetFileTime",
            "OutputDebugStringA",
            "OutputDebugStringW",
            "GetShortPathNameA",
            "GetShortPathNameW",
            "GetLongPathNameA",
            "GetLongPathNameW",
            "VirtualProtect",
            "LocalAlloc",
            "LocalFree",
        ] {
            assert!(registry.resolve(&ApiKey::new(MODULE, name)).is_some());
        }
    }

    #[test]
    fn resolves_windows_paths_lexically() {
        assert_eq!(
            absolute_windows_path(r"C:\VNRT\game", r"..\data\.\script.bin").unwrap(),
            r"C:\VNRT\data\script.bin"
        );
    }
}
