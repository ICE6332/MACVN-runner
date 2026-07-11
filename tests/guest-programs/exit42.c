// The build temporarily compiles this branch into a stub DLL solely to let
// rust-lld create a Windows import library without an SDK installation.
#ifdef BUILD_KERNEL32_STUB
void ExitProcess(unsigned int code) {
    (void)code;
}
__declspec(dllexport) unsigned int __stdcall GetTickCount(void) {
    return 0;
}
__declspec(dllexport) void __stdcall GetStartupInfoA(void *info) {
    (void)info;
}
__declspec(dllexport) void __stdcall GetStartupInfoW(void *info) {
    (void)info;
}
__declspec(dllexport) unsigned int __stdcall GetVersion(void) {
    return 0;
}
__declspec(dllexport) int __stdcall GetVersionExA(void *info) {
    (void)info;
    return 0;
}
__declspec(dllexport) int __stdcall GetVersionExW(void *info) {
    (void)info;
    return 0;
}
__declspec(dllexport) int __stdcall QueryPerformanceCounter(
    unsigned long long *value) {
    (void)value;
    return 0;
}
__declspec(dllexport) int __stdcall QueryPerformanceFrequency(
    unsigned long long *value) {
    (void)value;
    return 0;
}
__declspec(dllexport) void __stdcall GetSystemTimeAsFileTime(
    unsigned long long *value) {
    (void)value;
}
__declspec(dllexport) void *__stdcall GetProcessHeap(void) {
    return (void *)0;
}
__declspec(dllexport) void *__stdcall HeapCreate(unsigned int options,
                                                 unsigned int initial_size,
                                                 unsigned int maximum_size) {
    (void)options;
    (void)initial_size;
    (void)maximum_size;
    return (void *)0;
}
__declspec(dllexport) int __stdcall HeapDestroy(void *heap) {
    (void)heap;
    return 0;
}
__declspec(dllexport) void *__stdcall HeapAlloc(void *heap, unsigned int flags,
                                                unsigned int size) {
    (void)heap;
    (void)flags;
    (void)size;
    return (void *)0;
}
__declspec(dllexport) void *__stdcall HeapReAlloc(void *heap,
                                                  unsigned int flags,
                                                  void *memory,
                                                  unsigned int size) {
    (void)heap;
    (void)flags;
    (void)memory;
    (void)size;
    return (void *)0;
}
__declspec(dllexport) int __stdcall HeapFree(void *heap, unsigned int flags,
                                             void *memory) {
    (void)heap;
    (void)flags;
    (void)memory;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall HeapSize(void *heap,
                                                       unsigned int flags,
                                                       const void *memory) {
    (void)heap;
    (void)flags;
    (void)memory;
    return 0;
}
__declspec(dllexport) void __stdcall InitializeCriticalSection(void *section) {
    (void)section;
}
__declspec(dllexport) void __stdcall DeleteCriticalSection(void *section) {
    (void)section;
}
__declspec(dllexport) void __stdcall EnterCriticalSection(void *section) {
    (void)section;
}
__declspec(dllexport) void __stdcall LeaveCriticalSection(void *section) {
    (void)section;
}
__declspec(dllexport) unsigned int __stdcall TlsAlloc(void) {
    return 0xffffffffU;
}
__declspec(dllexport) int __stdcall TlsFree(unsigned int index) {
    (void)index;
    return 0;
}
__declspec(dllexport) void *__stdcall TlsGetValue(unsigned int index) {
    (void)index;
    return (void *)0;
}
__declspec(dllexport) int __stdcall TlsSetValue(unsigned int index,
                                                void *value) {
    (void)index;
    (void)value;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall SetHandleCount(
    unsigned int count) {
    return count;
}
__declspec(dllexport) int __stdcall WideCharToMultiByte(
    unsigned int code_page, unsigned int flags, const unsigned short *input,
    int input_length, char *output, int output_length, const char *default_char,
    int *used_default) {
    (void)code_page; (void)flags; (void)input; (void)input_length;
    (void)output; (void)output_length; (void)default_char; (void)used_default;
    return 0;
}
__declspec(dllexport) int __stdcall MultiByteToWideChar(
    unsigned int code_page, unsigned int flags, const char *input,
    int input_length, unsigned short *output, int output_length) {
    (void)code_page; (void)flags; (void)input; (void)input_length;
    (void)output; (void)output_length;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall GetACP(void) { return 932; }
__declspec(dllexport) unsigned int __stdcall GetOEMCP(void) { return 932; }
__declspec(dllexport) int __stdcall GetCPInfo(unsigned int code_page,
                                              void *info) {
    (void)code_page; (void)info; return 0;
}
__declspec(dllexport) void __stdcall GetSystemInfo(void *info) { (void)info; }
__declspec(dllexport) int __stdcall GetStringTypeW(
    unsigned int info_type, const unsigned short *input, int length,
    unsigned short *types) {
    (void)info_type; (void)input; (void)length; (void)types; return 0;
}
__declspec(dllexport) int __stdcall GetStringTypeA(
    unsigned int locale, unsigned int info_type, const char *input, int length,
    unsigned short *types) {
    (void)locale; (void)info_type; (void)input; (void)length; (void)types;
    return 0;
}
__declspec(dllexport) int __stdcall LCMapStringW(
    unsigned int locale, unsigned int flags, const unsigned short *input,
    int input_length, unsigned short *output, int output_length) {
    (void)locale; (void)flags; (void)input; (void)input_length;
    (void)output; (void)output_length; return 0;
}
__declspec(dllexport) int __stdcall LCMapStringA(
    unsigned int locale, unsigned int flags, const char *input,
    int input_length, char *output, int output_length) {
    (void)locale; (void)flags; (void)input; (void)input_length;
    (void)output; (void)output_length; return 0;
}
__declspec(dllexport) int __stdcall IsProcessorFeaturePresent(
    unsigned int feature) {
    (void)feature; return 0;
}
__declspec(dllexport) void *__stdcall SetUnhandledExceptionFilter(void *filter) {
    (void)filter; return (void *)0;
}
__declspec(dllexport) void *__stdcall CreateMutexA(void *security,
                                                   int initial_owner,
                                                   const char *name) {
    (void)security; (void)initial_owner; (void)name; return (void *)0;
}
__declspec(dllexport) int __stdcall ReleaseMutex(void *mutex) {
    (void)mutex; return 0;
}
__declspec(dllexport) int __stdcall SetCurrentDirectoryA(const char *path) {
    (void)path; return 0;
}
__declspec(dllexport) void *__stdcall GetModuleHandleA(const char *name) {
    (void)name;
    return (void *)0;
}
__declspec(dllexport) void *__stdcall GetModuleHandleW(const unsigned short *name) {
    (void)name;
    return (void *)0;
}
__declspec(dllexport) void *__stdcall GetProcAddress(void *module,
                                                    const char *name) {
    (void)module;
    (void)name;
    return (void *)0;
}
__declspec(dllexport) char *__stdcall GetCommandLineA(void) {
    return (char *)0;
}
__declspec(dllexport) unsigned short *__stdcall GetCommandLineW(void) {
    return (unsigned short *)0;
}
__declspec(dllexport) unsigned int __stdcall GetModuleFileNameA(
    void *module, char *output, unsigned int size) {
    (void)module;
    (void)output;
    (void)size;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall GetModuleFileNameW(
    void *module, unsigned short *output, unsigned int size) {
    (void)module;
    (void)output;
    (void)size;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall GetLastError(void) {
    return 0;
}
__declspec(dllexport) void __stdcall SetLastError(unsigned int value) {
    (void)value;
}
__declspec(dllexport) void *__stdcall CreateFileA(
    const char *path, unsigned int access, unsigned int share,
    void *security, unsigned int creation, unsigned int flags, void *template_file) {
    (void)path;
    (void)access;
    (void)share;
    (void)security;
    (void)creation;
    (void)flags;
    (void)template_file;
    return (void *)0;
}
__declspec(dllexport) void *__stdcall CreateFileW(
    const unsigned short *path, unsigned int access, unsigned int share,
    void *security, unsigned int creation, unsigned int flags, void *template_file) {
    (void)path;
    (void)access;
    (void)share;
    (void)security;
    (void)creation;
    (void)flags;
    (void)template_file;
    return (void *)0;
}
__declspec(dllexport) int __stdcall ReadFile(
    void *file, void *output, unsigned int length, unsigned int *bytes_read,
    void *overlapped) {
    (void)file;
    (void)output;
    (void)length;
    (void)bytes_read;
    (void)overlapped;
    return 0;
}
__declspec(dllexport) int __stdcall CloseHandle(void *handle) {
    (void)handle;
    return 0;
}
__declspec(dllexport) void *__stdcall GetStdHandle(int selector) {
    (void)selector;
    return (void *)0;
}
__declspec(dllexport) int __stdcall WriteFile(
    void *file, const void *input, unsigned int length,
    unsigned int *bytes_written, void *overlapped) {
    (void)file;
    (void)input;
    (void)length;
    (void)bytes_written;
    (void)overlapped;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall GetFileSize(
    void *file, unsigned int *high_size) {
    (void)file;
    (void)high_size;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall SetFilePointer(
    void *file, int distance, int *high_distance, unsigned int origin) {
    (void)file;
    (void)distance;
    (void)high_distance;
    (void)origin;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall GetFileType(void *file) {
    (void)file;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall GetEnvironmentVariableA(
    const char *name, char *output, unsigned int size) {
    (void)name;
    (void)output;
    (void)size;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall GetEnvironmentVariableW(
    const unsigned short *name, unsigned short *output, unsigned int size) {
    (void)name;
    (void)output;
    (void)size;
    return 0;
}
__declspec(dllexport) char *__stdcall GetEnvironmentStringsA(void) {
    return (char *)0;
}
__declspec(dllexport) unsigned short *__stdcall GetEnvironmentStringsW(void) {
    return (unsigned short *)0;
}
__declspec(dllexport) int __stdcall FreeEnvironmentStringsA(char *environment) {
    (void)environment;
    return 0;
}
__declspec(dllexport) int __stdcall FreeEnvironmentStringsW(
    unsigned short *environment) {
    (void)environment;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall GetCurrentDirectoryA(
    unsigned int size, char *output) {
    (void)size;
    (void)output;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall GetCurrentDirectoryW(
    unsigned int size, unsigned short *output) {
    (void)size;
    (void)output;
    return 0;
}
__declspec(dllexport) unsigned int __stdcall GetCurrentProcessId(void) {
    return 0;
}
__declspec(dllexport) unsigned int __stdcall GetCurrentThreadId(void) {
    return 0;
}
__declspec(dllexport) void *__stdcall VirtualAlloc(void *address, unsigned int size,
                                                   unsigned int allocation_type,
                                                   unsigned int protection) {
    (void)address;
    (void)size;
    (void)allocation_type;
    (void)protection;
    return (void *)0;
}
__declspec(dllexport) int __stdcall VirtualFree(void *address, unsigned int size,
                                                unsigned int free_type) {
    (void)address;
    (void)size;
    (void)free_type;
    return 0;
}
#else

// Freestanding compiler-generated guest used to grow the x86 interpreter.
// Keep this independent of the C runtime and Windows SDK headers.
// ExitProcess never returns, so cdecl vs stdcall has no observable stack-cleanup
// difference in this fixture. Using cdecl keeps the SDK-free import name plain.

typedef struct TlsDirectory32 {
    const void *start_address_of_raw_data;
    const void *end_address_of_raw_data;
    unsigned int *address_of_index;
    void (__stdcall **address_of_callbacks)(void *, unsigned int, void *);
    unsigned int size_of_zero_fill;
    unsigned int characteristics;
} TlsDirectory32;

#pragma section(".tls$AAA", read, write)
__declspec(allocate(".tls$AAA")) unsigned int tls_template_start = 0x13579bdfU;
#pragma section(".tls$ZZZ", read, write)
__declspec(allocate(".tls$ZZZ")) unsigned char tls_template_end = 0;
volatile unsigned int tls_index = 0xffffffffU;
volatile unsigned int tls_callback_state = 0;
volatile unsigned int tls_callback_module = 0;
volatile unsigned int tls_callback_reason = 0;
volatile unsigned int tls_callback_reserved = 1;
void __stdcall tls_callback_one(void *module, unsigned int reason,
                                void *reserved);
void __stdcall tls_callback_two(void *module, unsigned int reason,
                                void *reserved);
#pragma section(".CRT$XLA", read)
__declspec(allocate(".CRT$XLA")) void (__stdcall *tls_callbacks[])(
    void *, unsigned int, void *) = {tls_callback_one, tls_callback_two,
                                     (void *)0};
#pragma section(".rdata$T", read)
__declspec(allocate(".rdata$T")) const TlsDirectory32 _tls_used = {
    &tls_template_start, &tls_template_end, (unsigned int *)&tls_index,
    tls_callbacks, 8, 0};

__declspec(dllimport) __declspec(noreturn) void ExitProcess(unsigned int code);
__declspec(dllimport) void __stdcall GetStartupInfoA(void *info);
__declspec(dllimport) void __stdcall GetStartupInfoW(void *info);
__declspec(dllimport) unsigned int __stdcall GetVersion(void);
__declspec(dllimport) int __stdcall GetVersionExA(void *info);
__declspec(dllimport) int __stdcall GetVersionExW(void *info);
__declspec(dllimport) int __stdcall QueryPerformanceCounter(
    unsigned long long *value);
__declspec(dllimport) int __stdcall QueryPerformanceFrequency(
    unsigned long long *value);
__declspec(dllimport) void __stdcall GetSystemTimeAsFileTime(
    unsigned long long *value);
__declspec(dllimport) void *__stdcall GetProcessHeap(void);
__declspec(dllimport) void *__stdcall HeapCreate(unsigned int options,
                                                 unsigned int initial_size,
                                                 unsigned int maximum_size);
__declspec(dllimport) int __stdcall HeapDestroy(void *heap);
__declspec(dllimport) void *__stdcall HeapAlloc(void *heap, unsigned int flags,
                                                unsigned int size);
__declspec(dllimport) void *__stdcall HeapReAlloc(void *heap,
                                                  unsigned int flags,
                                                  void *memory,
                                                  unsigned int size);
__declspec(dllimport) int __stdcall HeapFree(void *heap, unsigned int flags,
                                             void *memory);
__declspec(dllimport) unsigned int __stdcall HeapSize(void *heap,
                                                       unsigned int flags,
                                                       const void *memory);
__declspec(dllimport) void __stdcall InitializeCriticalSection(void *section);
__declspec(dllimport) void __stdcall DeleteCriticalSection(void *section);
__declspec(dllimport) void __stdcall EnterCriticalSection(void *section);
__declspec(dllimport) void __stdcall LeaveCriticalSection(void *section);
__declspec(dllimport) unsigned int __stdcall TlsAlloc(void);
__declspec(dllimport) int __stdcall TlsFree(unsigned int index);
__declspec(dllimport) void *__stdcall TlsGetValue(unsigned int index);
__declspec(dllimport) int __stdcall TlsSetValue(unsigned int index, void *value);
__declspec(dllimport) unsigned int __stdcall SetHandleCount(unsigned int count);
__declspec(dllimport) int __stdcall WideCharToMultiByte(
    unsigned int code_page, unsigned int flags, const unsigned short *input,
    int input_length, char *output, int output_length, const char *default_char,
    int *used_default);
__declspec(dllimport) int __stdcall MultiByteToWideChar(
    unsigned int code_page, unsigned int flags, const char *input,
    int input_length, unsigned short *output, int output_length);
__declspec(dllimport) unsigned int __stdcall GetACP(void);
__declspec(dllimport) unsigned int __stdcall GetOEMCP(void);
__declspec(dllimport) int __stdcall GetCPInfo(unsigned int code_page, void *info);
__declspec(dllimport) void __stdcall GetSystemInfo(void *info);
__declspec(dllimport) int __stdcall GetStringTypeW(
    unsigned int info_type, const unsigned short *input, int length,
    unsigned short *types);
__declspec(dllimport) int __stdcall GetStringTypeA(
    unsigned int locale, unsigned int info_type, const char *input, int length,
    unsigned short *types);
__declspec(dllimport) int __stdcall LCMapStringW(
    unsigned int locale, unsigned int flags, const unsigned short *input,
    int input_length, unsigned short *output, int output_length);
__declspec(dllimport) int __stdcall LCMapStringA(
    unsigned int locale, unsigned int flags, const char *input,
    int input_length, char *output, int output_length);
__declspec(dllimport) int __stdcall IsProcessorFeaturePresent(
    unsigned int feature);
__declspec(dllimport) void *__stdcall SetUnhandledExceptionFilter(void *filter);
__declspec(dllimport) void *__stdcall CreateMutexA(void *security,
                                                   int initial_owner,
                                                   const char *name);
__declspec(dllimport) int __stdcall ReleaseMutex(void *mutex);
__declspec(dllimport) int __stdcall SetCurrentDirectoryA(const char *path);
__declspec(dllimport) void *__stdcall GetModuleHandleA(const char *name);
__declspec(dllimport) void *__stdcall GetModuleHandleW(const unsigned short *name);
__declspec(dllimport) void *__stdcall GetProcAddress(void *module,
                                                    const char *name);
__declspec(dllimport) char *__stdcall GetCommandLineA(void);
__declspec(dllimport) unsigned short *__stdcall GetCommandLineW(void);
__declspec(dllimport) unsigned int __stdcall GetModuleFileNameA(
    void *module, char *output, unsigned int size);
__declspec(dllimport) unsigned int __stdcall GetModuleFileNameW(
    void *module, unsigned short *output, unsigned int size);
__declspec(dllimport) unsigned int __stdcall GetLastError(void);
__declspec(dllimport) void __stdcall SetLastError(unsigned int value);
__declspec(dllimport) void *__stdcall CreateFileA(
    const char *path, unsigned int access, unsigned int share,
    void *security, unsigned int creation, unsigned int flags, void *template_file);
__declspec(dllimport) void *__stdcall CreateFileW(
    const unsigned short *path, unsigned int access, unsigned int share,
    void *security, unsigned int creation, unsigned int flags, void *template_file);
__declspec(dllimport) int __stdcall ReadFile(
    void *file, void *output, unsigned int length, unsigned int *bytes_read,
    void *overlapped);
__declspec(dllimport) int __stdcall CloseHandle(void *handle);
__declspec(dllimport) void *__stdcall GetStdHandle(int selector);
__declspec(dllimport) int __stdcall WriteFile(
    void *file, const void *input, unsigned int length,
    unsigned int *bytes_written, void *overlapped);
__declspec(dllimport) unsigned int __stdcall GetFileSize(
    void *file, unsigned int *high_size);
__declspec(dllimport) unsigned int __stdcall SetFilePointer(
    void *file, int distance, int *high_distance, unsigned int origin);
__declspec(dllimport) unsigned int __stdcall GetFileType(void *file);
__declspec(dllimport) unsigned int __stdcall GetEnvironmentVariableA(
    const char *name, char *output, unsigned int size);
__declspec(dllimport) unsigned int __stdcall GetEnvironmentVariableW(
    const unsigned short *name, unsigned short *output, unsigned int size);
__declspec(dllimport) char *__stdcall GetEnvironmentStringsA(void);
__declspec(dllimport) unsigned short *__stdcall GetEnvironmentStringsW(void);
__declspec(dllimport) int __stdcall FreeEnvironmentStringsA(char *environment);
__declspec(dllimport) int __stdcall FreeEnvironmentStringsW(
    unsigned short *environment);
__declspec(dllimport) unsigned int __stdcall GetCurrentDirectoryA(
    unsigned int size, char *output);
__declspec(dllimport) unsigned int __stdcall GetCurrentDirectoryW(
    unsigned int size, unsigned short *output);
__declspec(dllimport) unsigned int __stdcall GetCurrentProcessId(void);
__declspec(dllimport) unsigned int __stdcall GetCurrentThreadId(void);
__declspec(dllimport) void *__stdcall VirtualAlloc(void *address, unsigned int size,
                                                   unsigned int allocation_type,
                                                   unsigned int protection);
__declspec(dllimport) int __stdcall VirtualFree(void *address, unsigned int size,
                                                unsigned int free_type);

typedef unsigned int(__stdcall *GetTickCountFunction)(void);

__declspec(noinline) unsigned int read_teb_self(void) {
    unsigned int value;
    __asm__ volatile("movl %%fs:0x18, %0" : "=r"(value));
    return value;
}

__declspec(noinline) unsigned int read_process_environment_block(void) {
    unsigned int value;
    __asm__ volatile("movl %%fs:0x30, %0" : "=r"(value));
    return value;
}

__declspec(noinline) unsigned int read_teb_last_error(void) {
    unsigned int value;
    __asm__ volatile("movl %%fs:0x34, %0" : "=r"(value));
    return value;
}

__declspec(noinline) unsigned int read_teb_tls_slots(void) {
    unsigned int value;
    __asm__ volatile("movl %%fs:0x2c, %0" : "=r"(value));
    return value;
}

void __stdcall tls_callback_one(void *module, unsigned int reason,
                                void *reserved) {
    unsigned int slots = read_teb_tls_slots();
    unsigned char *data = slots == 0 ? (unsigned char *)0
                                     : *(unsigned char **)slots;
    tls_callback_module = (unsigned int)module;
    tls_callback_reason = reason;
    tls_callback_reserved = (unsigned int)reserved;
    if (tls_callback_state == 0 && data != (void *)0 &&
        *(unsigned int *)data == 0x13579bdfU) {
        data[6] = 0x66;
        tls_callback_state = 1;
    } else {
        tls_callback_state = 0xff;
    }
}

void __stdcall tls_callback_two(void *module, unsigned int reason,
                                void *reserved) {
    unsigned int slots = read_teb_tls_slots();
    unsigned char *data = slots == 0 ? (unsigned char *)0
                                     : *(unsigned char **)slots;
    if (tls_callback_state == 1 && data != (void *)0 && data[6] == 0x66 &&
        (unsigned int)module == tls_callback_module &&
        reason == tls_callback_reason &&
        (unsigned int)reserved == tls_callback_reserved) {
        tls_callback_state = 2;
    } else {
        tls_callback_state = 0xff;
    }
}

__declspec(noinline) void write_teb_last_error(unsigned int value) {
    __asm__ volatile("movl %0, %%fs:0x34" : : "r"(value) : "memory");
}

__declspec(noinline) void *memset(void *destination, int value,
                                  unsigned int length) {
    unsigned char *cursor = (unsigned char *)destination;
    while (length != 0) {
        *cursor = (unsigned char)value;
        cursor += 1;
        length -= 1;
    }
    return destination;
}

__declspec(noinline) int validate_loader_chain(
    unsigned int list_head, unsigned int link_offset, unsigned int main_module,
    unsigned int kernel32_module, unsigned int user32_module) {
    unsigned int current = *(unsigned int *)list_head;
    unsigned int previous = list_head;
    unsigned int seen = 0;
    unsigned int count = 0;
    while (current != list_head) {
        if (count >= 3 || *(unsigned int *)(current + 4) != previous) {
            return 0;
        }
        unsigned int entry = current - link_offset;
        unsigned int module_base = *(unsigned int *)(entry + 0x18);
        unsigned short *full_name = *(unsigned short **)(entry + 0x28);
        unsigned short *base_name = *(unsigned short **)(entry + 0x30);
        if (full_name == (void *)0 || base_name == (void *)0 ||
            full_name[0] != 'C') {
            return 0;
        }
        if (module_base == main_module) {
            if (base_name[0] == 0) {
                return 0;
            }
            seen |= 1;
        } else if (module_base == kernel32_module) {
            if (base_name[0] != 'k') {
                return 0;
            }
            seen |= 2;
        } else if (module_base == user32_module) {
            if (base_name[0] != 'u') {
                return 0;
            }
            seen |= 4;
        } else {
            return 0;
        }
        previous = current;
        current = *(unsigned int *)current;
        count += 1;
    }
    return count == 3 && seen == 7 &&
           *(unsigned int *)(list_head + 4) == previous &&
           *(unsigned int *)previous == list_head;
}

volatile unsigned int arithmetic_unsigned_value = 1234567U;
volatile unsigned int arithmetic_unsigned_divisor = 37U;
volatile int arithmetic_signed_value = -1234567;
volatile int arithmetic_signed_divisor = 37;

__declspec(noinline) int validate_integer_arithmetic(
    unsigned int value, unsigned int divisor, int signed_value,
    int signed_divisor) {
    volatile unsigned int product = value * divisor;
    volatile unsigned int quotient = product / divisor;
    volatile unsigned int remainder = product % divisor;
    volatile unsigned int logical_shift = value >> (divisor & 31);
    volatile int signed_quotient = signed_value / signed_divisor;
    volatile int signed_remainder = signed_value % signed_divisor;
    volatile int arithmetic_shift = signed_value >> (divisor & 31);
    volatile int negated = -signed_quotient;
    return product == 45678979U && quotient == 1234567U && remainder == 0 &&
           logical_shift == 38580U && signed_quotient == -33366 &&
           signed_remainder == -25 && arithmetic_shift == -38581 &&
           negated == 33366;
}

__declspec(noinline) int validate_string_operations(void) {
    unsigned char source[8] = {1, 2, 3, 4, 5, 6, 7, 8};
    unsigned char forward[8] = {0};
    unsigned char reverse[8] = {0};
    const unsigned char *source_cursor = source;
    unsigned char *destination_cursor = forward;
    unsigned int count = 8;
    __asm__ volatile("cld\n\trep movsb"
                     : "+S"(source_cursor), "+D"(destination_cursor),
                       "+c"(count)
                     :
                     : "memory", "cc");
    if (count != 0 || forward[0] != 1 || forward[7] != 8) {
        return 0;
    }

    source_cursor = source + 7;
    destination_cursor = reverse + 7;
    count = 8;
    __asm__ volatile("std\n\trep movsb\n\tcld"
                     : "+S"(source_cursor), "+D"(destination_cursor),
                       "+c"(count)
                     :
                     : "memory", "cc");
    if (count != 0 || reverse[0] != 1 || reverse[7] != 8) {
        return 0;
    }

    unsigned int words[3] = {0};
    unsigned int *word_cursor = words;
    count = 3;
    __asm__ volatile("cld\n\trep stosl"
                     : "+D"(word_cursor), "+c"(count)
                     : "a"(0x89abcdefU)
                     : "memory", "cc");
    if (words[0] != 0x89abcdefU || words[2] != 0x89abcdefU) {
        return 0;
    }

    destination_cursor = forward;
    count = 8;
    __asm__ volatile("cld\n\trepne scasb"
                     : "+D"(destination_cursor), "+c"(count)
                     : "a"(5)
                     : "memory", "cc");
    if (count != 3 || destination_cursor != forward + 5) {
        return 0;
    }

    forward[6] = 0xff;
    source_cursor = source;
    destination_cursor = forward;
    count = 8;
    __asm__ volatile("cld\n\trepe cmpsb"
                     : "+S"(source_cursor), "+D"(destination_cursor),
                       "+c"(count)
                     :
                     : "memory", "cc");
    if (count != 1 || source_cursor != source + 7 ||
        destination_cursor != forward + 7) {
        return 0;
    }

    unsigned int loaded;
    source_cursor = source + 3;
    __asm__ volatile("cld\n\tlodsb"
                     : "=a"(loaded), "+S"(source_cursor)
                     :
                     : "memory", "cc");
    return (unsigned char)loaded == 4 && source_cursor == source + 4;
}

__declspec(noinline) int validate_windows_10_services(void) {
    unsigned char startup_a[68] = {0};
    unsigned char startup_w[68] = {0};
    GetStartupInfoA(startup_a);
    GetStartupInfoW(startup_w);
    if (*(unsigned int *)(startup_a + 0x00) != 68 ||
        *(unsigned int *)(startup_w + 0x00) != 68 ||
        (*(unsigned int *)(startup_a + 0x2c) & 0x100U) == 0 ||
        (*(unsigned int *)(startup_w + 0x2c) & 0x100U) == 0 ||
        *(void **)(startup_a + 0x38) != GetStdHandle(-10) ||
        *(void **)(startup_a + 0x3c) != GetStdHandle(-11) ||
        *(void **)(startup_a + 0x40) != GetStdHandle(-12) ||
        *(void **)(startup_w + 0x38) != GetStdHandle(-10) ||
        *(void **)(startup_w + 0x3c) != GetStdHandle(-11) ||
        *(void **)(startup_w + 0x40) != GetStdHandle(-12)) {
        return 0;
    }

    unsigned int packed_version = GetVersion();
    if ((packed_version & 0xffU) != 10 ||
        ((packed_version >> 8) & 0xffU) != 0 ||
        (packed_version >> 16) != 19045U) {
        return 0;
    }

    unsigned char version_a[156] = {0};
    unsigned char version_w[284] = {0};
    *(unsigned int *)version_a = 156;
    *(unsigned int *)version_w = 284;
    if (!GetVersionExA(version_a) || !GetVersionExW(version_w) ||
        *(unsigned int *)(version_a + 0x04) != 10 ||
        *(unsigned int *)(version_a + 0x08) != 0 ||
        *(unsigned int *)(version_a + 0x0c) != 19045U ||
        *(unsigned int *)(version_a + 0x10) != 2 || version_a[154] != 1 ||
        *(unsigned int *)(version_w + 0x04) != 10 ||
        *(unsigned int *)(version_w + 0x08) != 0 ||
        *(unsigned int *)(version_w + 0x0c) != 19045U ||
        *(unsigned int *)(version_w + 0x10) != 2 || version_w[282] != 1) {
        return 0;
    }

    unsigned long long frequency = 0;
    unsigned long long counter_before = 0;
    unsigned long long counter_after = 0;
    unsigned long long filetime = 0;
    unsigned short wide_source[5];
    wide_source[0] = 'V';
    wide_source[1] = 'N';
    wide_source[2] = 'R';
    wide_source[3] = 'T';
    wide_source[4] = 0;
    char narrow[5];
    unsigned short wide_output[5];
    unsigned char code_page_info[18];
    unsigned int system_info[9];
    unsigned short type_source[3];
    unsigned short wide_types[3];
    unsigned short ansi_types[2];
    unsigned short mapped_wide[2];
    char mapped_ansi[2];
    if (WideCharToMultiByte(0, 0, wide_source, -1, (void *)0, 0,
                            (void *)0, (void *)0) != 5 ||
        WideCharToMultiByte(0, 0, wide_source, -1, narrow, 5,
                            (void *)0, (void *)0) != 5 ||
        narrow[0] != 'V' || narrow[4] != 0 ||
        MultiByteToWideChar(0, 0, narrow, -1, (void *)0, 0) != 5 ||
        MultiByteToWideChar(0, 0, narrow, -1, wide_output, 5) != 5 ||
        wide_output[0] != 'V' || wide_output[4] != 0) {
        return 0;
    }
    type_source[0] = 'A';
    type_source[1] = 0x3042;
    type_source[2] = 0;
    if (SetUnhandledExceptionFilter((void *)0x12345678U) != (void *)0 ||
        SetUnhandledExceptionFilter((void *)0) != (void *)0x12345678U ||
        IsProcessorFeaturePresent(6) != 0 ||
        LCMapStringW(0x0411, 0x100, type_source, 1, (void *)0, 0) != 1 ||
        LCMapStringW(0x0411, 0x100, type_source, 1, mapped_wide, 2) != 1 ||
        mapped_wide[0] != 'a' ||
        LCMapStringA(0x0411, 0x200, "a", 1, (void *)0, 0) != 1 ||
        LCMapStringA(0x0411, 0x200, "a", 1, mapped_ansi, 2) != 1 ||
        mapped_ansi[0] != 'A') {
        return 0;
    }
    if (GetACP() != 932 || GetOEMCP() != 932 ||
        !GetCPInfo(932, code_page_info) ||
        *(unsigned int *)code_page_info != 2 || code_page_info[4] != '?' ||
        code_page_info[6] != 0x81 || code_page_info[7] != 0x9f) {
        return 0;
    }
    GetSystemInfo(system_info);
    if (system_info[1] != 4096 || system_info[4] != 1 ||
        system_info[5] != 1 || system_info[7] != 65536) {
        return 0;
    }
    if (!GetStringTypeW(1, type_source, 3, wide_types) ||
        (wide_types[0] & 0x0181U) != 0x0181U ||
        (wide_types[1] & 0x0100U) == 0 ||
        (wide_types[2] & 0x0020U) == 0 ||
        !GetStringTypeA(0x0411, 1, "A1", 2, ansi_types) ||
        (ansi_types[0] & 0x0101U) != 0x0101U ||
        (ansi_types[1] & 0x0004U) == 0) {
        return 0;
    }
    if (SetHandleCount(64) != 64 || !QueryPerformanceFrequency(&frequency) ||
        !QueryPerformanceCounter(&counter_before) ||
        !QueryPerformanceCounter(&counter_after) ||
        frequency != 1000000000ULL || counter_after < counter_before) {
        return 0;
    }
    GetSystemTimeAsFileTime(&filetime);
    return filetime != 0;
}

__declspec(noinline) int validate_private_heap(void) {
    void *heap = HeapCreate(0, 16, 128);
    if (heap == (void *)0 || heap == GetProcessHeap()) {
        return 0;
    }
    unsigned char *memory = (unsigned char *)HeapAlloc(heap, 8, 8);
    if (memory == (void *)0 || HeapSize(heap, 0, memory) != 8 ||
        memory[0] != 0 || memory[7] != 0) {
        return 0;
    }
    memory[0] = 0x5a;
    memory[7] = 0xa5;
    memory = (unsigned char *)HeapReAlloc(heap, 8, memory, 16);
    if (memory == (void *)0 || HeapSize(heap, 0, memory) != 16 ||
        memory[0] != 0x5a || memory[7] != 0xa5 || memory[8] != 0 ||
        memory[15] != 0) {
        return 0;
    }
    return HeapFree(heap, 0, memory) && HeapDestroy(heap);
}

__declspec(noinline) int validate_critical_section(void) {
    unsigned int section[6];
    section[0] = 0xffffffffU;
    section[1] = 0xffffffffU;
    section[2] = 0xffffffffU;
    section[3] = 0xffffffffU;
    section[4] = 0xffffffffU;
    section[5] = 0xffffffffU;
    InitializeCriticalSection(section);
    if (section[1] != 0xffffffffU || section[2] != 0 || section[3] != 0) {
        return 0;
    }
    EnterCriticalSection(section);
    EnterCriticalSection(section);
    if (section[2] != 2 || section[3] != GetCurrentThreadId()) {
        return 0;
    }
    LeaveCriticalSection(section);
    LeaveCriticalSection(section);
    if (section[1] != 0xffffffffU || section[2] != 0 || section[3] != 0) {
        return 0;
    }
    DeleteCriticalSection(section);
    return section[0] == 0 && section[5] == 0;
}

__declspec(noinline) int validate_dynamic_tls(void) {
    unsigned int index = TlsAlloc();
    if (index == 0xffffffffU || index == tls_index ||
        TlsGetValue(index) != (void *)0 ||
        !TlsSetValue(index, (void *)0x2468ace0U) ||
        TlsGetValue(index) != (void *)0x2468ace0U || !TlsFree(index)) {
        return 0;
    }
    SetLastError(0);
    return TlsGetValue(index) == (void *)0 && GetLastError() == 87;
}

__declspec(noinline) int validate_named_mutex(void) {
    void *mutex = CreateMutexA((void *)0, 1, "vnrt-fixture-mutex");
    if (mutex == (void *)0 || !ReleaseMutex(mutex) || !CloseHandle(mutex)) {
        return 0;
    }
    return 1;
}

__declspec(noinline) __declspec(noreturn) void guest_main(void) {
    unsigned int teb = read_teb_self();
    unsigned int peb = read_process_environment_block();
    void *main_module = GetModuleHandleA((void *)0);
    if (teb == 0 || peb == 0 || *(unsigned int *)(teb + 0x18) != teb ||
        *(unsigned int *)(teb + 0x30) != peb ||
        *(unsigned int *)(peb + 0x08) != (unsigned int)main_module ||
        *(unsigned int *)(teb + 0x20) != GetCurrentProcessId() ||
        *(unsigned int *)(teb + 0x24) != GetCurrentThreadId()) {
        ExitProcess(122);
    }
    unsigned int tls_slots = read_teb_tls_slots();
    if (tls_slots == 0 || tls_index != 0) {
        ExitProcess(127);
    }
    unsigned char *tls_data =
        *(unsigned char **)(tls_slots + tls_index * sizeof(void *));
    if (tls_data == (void *)0 || *(unsigned int *)tls_data != 0x13579bdfU ||
        tls_data[4] != 0 || tls_data[6] != 0x66 || tls_data[11] != 0 ||
        tls_callback_state != 2 || tls_callback_module != (unsigned int)main_module ||
        tls_callback_reason != 1 || tls_callback_reserved != 0) {
        ExitProcess(127);
    }
    tls_data[7] = 0x5a;
    if ((*(unsigned char **)(tls_slots))[7] != 0x5a) {
        ExitProcess(128);
    }
    if (!validate_integer_arithmetic(
            arithmetic_unsigned_value, arithmetic_unsigned_divisor,
            arithmetic_signed_value, arithmetic_signed_divisor)) {
        ExitProcess(129);
    }
    if (!validate_string_operations()) {
        ExitProcess(130);
    }
    if (!validate_windows_10_services()) {
        ExitProcess(131);
    }
    if (!validate_private_heap()) {
        ExitProcess(132);
    }
    if (!validate_critical_section()) {
        ExitProcess(133);
    }
    if (!validate_dynamic_tls()) {
        ExitProcess(134);
    }
    if (!validate_named_mutex()) {
        ExitProcess(135);
    }
    if (!SetCurrentDirectoryA("C:\\VNRT")) {
        ExitProcess(136);
    }
    unsigned int process_parameters = *(unsigned int *)(peb + 0x10);
    if (process_parameters == 0) {
        ExitProcess(124);
    }
    unsigned short *peb_current_directory =
        *(unsigned short **)(process_parameters + 0x28);
    unsigned short *peb_image_path =
        *(unsigned short **)(process_parameters + 0x3c);
    unsigned short *peb_command_line =
        *(unsigned short **)(process_parameters + 0x44);
    unsigned short *peb_environment =
        *(unsigned short **)(process_parameters + 0x48);
    if (*(unsigned int *)(peb + 0x18) != (unsigned int)GetProcessHeap() ||
        *(unsigned int *)(process_parameters + 0x1c) !=
            (unsigned int)GetStdHandle(-11) ||
        *(unsigned short *)(process_parameters + 0x40) == 0 ||
        peb_current_directory[0] != 'C' || peb_image_path[0] != 'C' ||
        peb_command_line[0] != '"' || peb_environment[0] != 'V') {
        ExitProcess(124);
    }
    void *kernel32 = GetModuleHandleA("kernel32.dll");
    if (kernel32 == (void *)0) {
        ExitProcess(104);
    }
    if (GetModuleHandleW(L"kernel32.dll") != kernel32) {
        ExitProcess(109);
    }
    void *user32 = GetModuleHandleA("user32.dll");
    unsigned int loader_data = *(unsigned int *)(peb + 0x0c);
    if (user32 == (void *)0 || loader_data == 0 ||
        !validate_loader_chain(loader_data + 0x0c, 0,
                               (unsigned int)main_module,
                               (unsigned int)kernel32, (unsigned int)user32) ||
        !validate_loader_chain(loader_data + 0x14, 8,
                               (unsigned int)main_module,
                               (unsigned int)kernel32, (unsigned int)user32) ||
        !validate_loader_chain(loader_data + 0x1c, 0x10,
                               (unsigned int)main_module,
                               (unsigned int)kernel32, (unsigned int)user32)) {
        ExitProcess(126);
    }
    GetTickCountFunction get_tick_count =
        (GetTickCountFunction)GetProcAddress(kernel32, "GetTickCount");
    if (get_tick_count == (void *)0) {
        ExitProcess(108);
    }
    if (GetProcAddress(kernel32, "VnrtDefinitelyMissing") != (void *)0 ||
        GetLastError() != 127) {
        ExitProcess(137);
    }
    volatile unsigned int ticks = get_tick_count();
    char *command_line_a = GetCommandLineA();
    unsigned short *command_line_w = GetCommandLineW();
    if (command_line_a == (void *)0 || command_line_a[0] != '"') {
        ExitProcess(110);
    }
    if (command_line_w == (void *)0 || command_line_w[0] != '"') {
        ExitProcess(111);
    }
    char module_path_a[64];
    unsigned short module_path_w[64];
    if (GetModuleFileNameA((void *)0, module_path_a, 64) == 0 ||
        module_path_a[0] != 'C') {
        ExitProcess(112);
    }
    if (GetModuleFileNameW((void *)0, module_path_w, 64) == 0 ||
        module_path_w[0] != 'C') {
        ExitProcess(113);
    }
    void *file_a = CreateFileA("resource.txt", 0x80000000U, 1, (void *)0, 3,
                               0, (void *)0);
    char file_data_a[8] = {0};
    unsigned int bytes_read_a = 0;
    if (file_a == (void *)-1 || GetFileType(file_a) != 1 ||
        GetFileSize(file_a, (void *)0) != 7 ||
        !ReadFile(file_a, file_data_a, 3, &bytes_read_a, (void *)0) ||
        bytes_read_a != 3 || file_data_a[0] != 'V' || file_data_a[2] != 'R' ||
        SetFilePointer(file_a, -4, (void *)0, 2) != 3 ||
        !ReadFile(file_a, file_data_a, 4, &bytes_read_a, (void *)0) ||
        bytes_read_a != 4 || file_data_a[0] != 'T' || file_data_a[3] != '\n' ||
        !CloseHandle(file_a)) {
        ExitProcess(115);
    }
    void *file_w = CreateFileW(L"resource.txt", 0x80000000U, 1, (void *)0, 3,
                               0, (void *)0);
    char file_data_w[8] = {0};
    unsigned int bytes_read_w = 0;
    if (file_w == (void *)-1 ||
        !ReadFile(file_w, file_data_w, 7, &bytes_read_w, (void *)0) ||
        bytes_read_w != 7 || file_data_w[6] != '\n' || !CloseHandle(file_w)) {
        ExitProcess(116);
    }
    void *standard_output = GetStdHandle(-11);
    unsigned int bytes_written = 0;
    if (standard_output == (void *)-1 || GetFileType(standard_output) != 2 ||
        !WriteFile(standard_output, "guest-ok\n", 9, &bytes_written, (void *)0) ||
        bytes_written != 9) {
        ExitProcess(117);
    }
    char environment_a[16] = {0};
    unsigned short environment_w[16] = {0};
    if (GetEnvironmentVariableA("vnrt_test", environment_a, 16) != 5 ||
        environment_a[0] != 'r' || environment_a[4] != 'y') {
        ExitProcess(118);
    }
    if (GetEnvironmentVariableW(L"VNRT_TEST", environment_w, 16) != 5 ||
        environment_w[0] != 'r' || environment_w[4] != 'y') {
        ExitProcess(119);
    }
    char *environment_block_a = GetEnvironmentStringsA();
    unsigned short *environment_block_w = GetEnvironmentStringsW();
    if (environment_block_a == (void *)0 || environment_block_a[0] != 'V' ||
        environment_block_w == (void *)0 || environment_block_w[0] != 'V' ||
        !FreeEnvironmentStringsA(environment_block_a) ||
        !FreeEnvironmentStringsW(environment_block_w)) {
        ExitProcess(125);
    }
    char current_directory_a[16] = {0};
    unsigned short current_directory_w[16] = {0};
    if (GetCurrentDirectoryA(16, current_directory_a) != 7 ||
        current_directory_a[0] != 'C' || current_directory_a[6] != 'T') {
        ExitProcess(120);
    }
    if (GetCurrentDirectoryW(16, current_directory_w) != 7 ||
        current_directory_w[0] != 'C' || current_directory_w[6] != 'T') {
        ExitProcess(121);
    }
    write_teb_last_error(4321);
    if (GetLastError() != 4321) {
        ExitProcess(114);
    }
    SetLastError(1234);
    if (read_teb_last_error() != 1234 || GetLastError() != 1234) {
        ExitProcess(123);
    }
    volatile unsigned int *virtual_cell =
        (volatile unsigned int *)VirtualAlloc((void *)0, 16, 0x3000, 0x04);
    if (virtual_cell == (void *)0) {
        ExitProcess(105);
    }
    if (*virtual_cell != 0) {
        ExitProcess(106);
    }
    volatile unsigned char base = 5;
    *virtual_cell = base;
    void *heap = GetProcessHeap();
    volatile unsigned int *cell =
        (volatile unsigned int *)HeapAlloc(heap, 8, sizeof(unsigned int));
    if (cell == (void *)0) {
        ExitProcess(100);
    }
    if (*cell != 0) {
        ExitProcess(103);
    }
    volatile short adjustment = -3;
    *cell = *virtual_cell;
    unsigned int result = *cell;
    result <<= 3;
    if (adjustment < 0) {
        result += 2;
    }
    if (!HeapFree(heap, 0, (void *)cell)) {
        ExitProcess(101);
    }
    if (!VirtualFree((void *)virtual_cell, 0, 0x8000)) {
        ExitProcess(107);
    }
    if (ticks == 0xffffffffU) {
        ExitProcess(102);
    }
    ExitProcess(result);
}

__declspec(noreturn) void mainCRTStartup(void) {
    guest_main();
}

#endif
