//! Composition layer for PE loading, x86 stepping, memory, and host calls.

mod allocation;
mod host_context;
mod loader;
mod objects;
mod process;

use allocation::*;
use host_context::*;
use loader::*;
use objects::*;
use process::*;

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, trace};
use vnrt_memory::{GuestAddress, GuestMemory, MemoryError, PAGE_SIZE_U32, Permissions};
use vnrt_pe::{Import, PeError, PeImage, Section};
use vnrt_win32::{
    ApiKey, ApiRegistry, FileEntry, Handle, HostCallContext, PROCESS_HEAP_HANDLE, ProcessIo,
    Win32Error,
};
use vnrt_x86::{CpuError, ExternalTargetResolver, Interpreter, Registers, StepOutcome};

/// External targets occupy a reserved, unmapped region near the top of the
/// 32-bit address space. Reaching one is intercepted before instruction fetch.
const HOST_THUNK_BASE: u32 = 0xfffe_0000;
const TLS_CALLBACK_RETURN_ADDRESS: u32 = 0xffff_fffc;
const HOST_CALLBACK_RETURN_ADDRESS: u32 = 0xffff_fff8;
const MAX_TLS_CALLBACKS: usize = 64;
const MAX_HOST_CALLBACK_ARGUMENTS: usize = 32;
const GUEST_STACK_BASE: u32 = 0x7fe0_0000;
const GUEST_STACK_SIZE: u32 = 0x0010_0000;
const GUEST_STACK_TOP: u32 = GUEST_STACK_BASE + GUEST_STACK_SIZE;
const GUEST_HEAP_BASE: u32 = 0x1000_0000;
const GUEST_HEAP_LIMIT: u32 = 0x7000_0000;
const GUEST_VIRTUAL_BASE: u32 = 0x7000_0000;
const GUEST_VIRTUAL_LIMIT: u32 = 0x7f00_0000;
const HOST_CALL_HISTORY_LIMIT: usize = 16;
const KERNEL32_MODULE_HANDLE: u32 = 0x7f10_0000;
const USER32_MODULE_HANDLE: u32 = 0x7f20_0000;
const HOST_MODULE_IMAGE_SIZE: u32 = 0x0001_0000;
const GUEST_PROCESS_DATA_BASE: u32 = 0x0fff_0000;
const GUEST_PROCESS_DATA_SIZE: u32 = 0x0001_0000;
const GUEST_TLS_BASE: u32 = 0x0ffe_0000;
const GUEST_TLS_SIZE: u32 = 0x0001_0000;
const GUEST_TLS_DATA_BASE: u32 = GUEST_TLS_BASE + PAGE_SIZE_U32;
const GUEST_PROCESS_PARAMETERS_BASE: u32 = GUEST_PROCESS_DATA_BASE + 0x0000_f000;
const GUEST_PROCESS_PARAMETERS_SIZE: u32 = 0x0000_1000;
const GUEST_TEB_BASE: u32 = 0x7ffd_e000;
const GUEST_PEB_BASE: u32 = 0x7ffd_f000;
const TEB_LAST_ERROR_OFFSET: u32 = 0x34;
const GUEST_PROCESS_ID: u32 = 0x1000;
const GUEST_THREAD_ID: u32 = 0x1004;
const STD_INPUT_HANDLE_VALUE: u32 = 0x0000_0200;
const STD_OUTPUT_HANDLE_VALUE: u32 = 0x0000_0204;
const STD_ERROR_HANDLE_VALUE: u32 = 0x0000_0208;
const FILE_TYPE_DISK: u32 = 1;
const FILE_TYPE_CHAR: u32 = 2;

/// Native platform operations needed by later user32 and audio implementations.
pub trait PlatformBackend {
    /// Poll native window and input events.
    fn poll_events(&mut self) -> Result<(), RuntimeError>;
    /// Submit interleaved stereo samples.
    fn submit_audio(&mut self, samples: &[f32]) -> Result<(), RuntimeError>;
}

/// Platform used by tests and the default command-line build.
#[derive(Debug, Default, Clone, Copy)]
pub struct HeadlessBackend;

impl PlatformBackend for HeadlessBackend {
    fn poll_events(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn submit_audio(&mut self, _samples: &[f32]) -> Result<(), RuntimeError> {
        Err(RuntimeError::Unsupported("headless audio output"))
    }
}

/// Re-export the selected SDL3 crate only when the native backend is requested.
/// Concrete window/audio ownership will be added behind this same feature.
#[cfg(feature = "sdl3-backend")]
pub use sdl3 as sdl3_backend;

/// Configurable safety limits for the execution loop.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunLimits {
    /// Maximum guest instructions/host transitions before yielding an error.
    pub max_instructions: u64,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self {
            max_instructions: 1_000_000,
        }
    }
}

/// Host-provided identity and command line for a newly loaded guest process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Win32-visible executable path.
    pub module_path: String,
    /// Complete Win32 command line including the executable name.
    pub command_line: String,
    /// Host directory exposed as the root of relative Guest file paths.
    pub filesystem_root: PathBuf,
    /// Win32-visible current directory, independent from the Host sandbox path.
    pub current_directory: String,
    /// Case-insensitive process environment visible through Kernel32.
    pub environment: BTreeMap<String, String>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            module_path: r"C:\VNRT\guest.exe".to_owned(),
            command_line: r#""C:\VNRT\guest.exe""#.to_owned(),
            filesystem_root: PathBuf::from("."),
            current_directory: r"C:\VNRT".to_owned(),
            environment: BTreeMap::from([("VNRT_TEST".to_owned(), "ready".to_owned())]),
        }
    }
}

/// Why the runtime stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// `ExitProcess` requested normal termination.
    Exited(u32),
    /// The CPU was explicitly halted.
    Halted,
}

/// Machine state captured for actionable execution-failure diagnostics.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticSnapshot {
    /// Register state at the failure boundary.
    pub registers: Registers,
    /// Linear base used for `FS:` memory operands, normally the current TEB.
    pub fs_base: u32,
    /// Up to 15 executable bytes beginning at EIP, when mapped.
    pub instruction_bytes: Vec<u8>,
    /// Return address followed by up to eight 32-bit stack arguments.
    pub stack_words: Vec<u32>,
    /// Most recent host calls, oldest first.
    pub recent_host_calls: Vec<ApiKey>,
}

/// Errors spanning loader, interpreter, memory, and host-call boundaries.
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// PE metadata is malformed or unsupported.
    #[error(transparent)]
    Pe(#[from] PeError),
    /// Guest address-space failure.
    #[error(transparent)]
    Memory(#[from] MemoryError),
    /// Decode or instruction execution failure.
    #[error(transparent)]
    Cpu(#[from] CpuError),
    /// Host-call failure.
    #[error(transparent)]
    Win32(#[from] Win32Error),
    /// A section range points outside the PE file.
    #[error("section {name} raw bytes are truncated")]
    TruncatedSection {
        /// PE section name.
        name: String,
    },
    /// The run limit was reached without termination.
    #[error("execution limit of {0} steps reached")]
    ExecutionLimit(u64),
    /// Known future runtime functionality.
    #[error("unsupported runtime feature: {0}")]
    Unsupported(&'static str),
}

/// Complete state of one guest process.
pub struct Runtime {
    /// x86 interpreter and architectural state.
    pub cpu: Interpreter,
    /// Sparse 32-bit guest address space.
    pub memory: GuestMemory,
    /// Win32 API implementations.
    pub api_registry: ApiRegistry,
    import_thunks: HashMap<GuestAddress, ApiKey>,
    exit_code: Option<u32>,
    started_at: Instant,
    image_base: GuestAddress,
    resource_directory: Option<(GuestAddress, u32)>,
    heaps: GuestHeapManager,
    global_allocations: BTreeMap<u32, u32>,
    tls_slots: TlsSlotManager,
    unhandled_exception_filter: u32,
    mutexes: MutexManager,
    events: EventManager,
    com_initialization_count: u32,
    cursor_display_count: i32,
    focused_window: u32,
    virtual_memory: GuestRegionAllocator,
    recent_host_calls: VecDeque<ApiKey>,
    host_modules: HashMap<String, GuestAddress>,
    command_line_ansi: GuestAddress,
    command_line_utf16: GuestAddress,
    module_path: String,
    last_error: u32,
    process_io: ProcessIo,
    guest_stdout: Vec<u8>,
    guest_stderr: Vec<u8>,
    environment: BTreeMap<String, String>,
    current_directory: String,
    environment_block_ansi: GuestAddress,
    environment_block_utf16: GuestAddress,
    main_entry_point: GuestAddress,
    pending_tls_callbacks: VecDeque<GuestAddress>,
    tls_callbacks_active: bool,
    suspended_host_calls: Vec<SuspendedHostCall>,
    guest_callback_targets: HashMap<u32, GuestAddress>,
}

#[derive(Debug)]
struct GuestCallback {
    target: GuestAddress,
    arguments: Vec<u32>,
}

#[derive(Debug)]
struct SuspendedHostCall {
    return_address: u32,
    resumed_stack_pointer: u32,
    callback_stack_pointer: u32,
    return_value: u32,
    capture_callback_return: bool,
    callbacks: VecDeque<GuestCallback>,
}

impl Runtime {
    /// Parse and load a PE32 image at its preferred base.
    pub fn load(bytes: &[u8], api_registry: ApiRegistry) -> Result<Self, RuntimeError> {
        Self::load_with_config(bytes, api_registry, RuntimeConfig::default())
    }

    /// Parse and load a PE32 image with explicit process identity strings.
    pub fn load_with_config(
        bytes: &[u8],
        api_registry: ApiRegistry,
        config: RuntimeConfig,
    ) -> Result<Self, RuntimeError> {
        let image = vnrt_pe::parse(bytes)?;
        let mut memory = GuestMemory::new();
        map_image(&mut memory, bytes, &image)?;
        let import_thunks = bind_imports(&mut memory, &image)?;
        let mut import_thunks = import_thunks;
        initialize_host_module_image(
            &mut memory,
            &api_registry,
            &mut import_thunks,
            "kernel32.dll",
            GuestAddress(KERNEL32_MODULE_HANDLE),
        )?;
        initialize_host_module_image(
            &mut memory,
            &api_registry,
            &mut import_thunks,
            "user32.dll",
            GuestAddress(USER32_MODULE_HANDLE),
        )?;
        let tls = initialize_static_tls(&mut memory, &image)?;
        protect_image(&mut memory, &image)?;
        memory.map_range(
            GuestAddress(GUEST_STACK_BASE),
            GUEST_STACK_SIZE,
            Permissions::READ_WRITE,
        )?;
        let process_data = initialize_process_data(
            &mut memory,
            &config,
            GuestAddress(image.optional.image_base),
            GuestAddress(image.entry_point()),
            image.optional.size_of_image,
        )?;
        initialize_win32_process_structures(
            &mut memory,
            GuestAddress(image.optional.image_base),
            process_data.process_parameters,
            process_data.loader_data,
            tls.slots,
        )?;
        let main_entry_point = GuestAddress(image.entry_point());
        let resource_directory = image
            .optional
            .data_directories
            .get(2)
            .filter(|directory| !directory.is_empty())
            .and_then(|directory| {
                image
                    .optional
                    .image_base
                    .checked_add(directory.rva)
                    .map(|address| (GuestAddress(address), directory.size))
            });
        let mut cpu = Interpreter::new(main_entry_point);
        cpu.state.registers.esp = GUEST_STACK_TOP;
        cpu.state.fs_base = GUEST_TEB_BASE;
        let mut pending_tls_callbacks = VecDeque::from(tls.callbacks);
        let tls_callbacks_active = if let Some(callback) = pending_tls_callbacks.pop_front() {
            enter_tls_callback(
                &mut cpu,
                &mut memory,
                callback,
                GuestAddress(image.optional.image_base),
            )?;
            true
        } else {
            false
        };
        let host_modules = HashMap::from([
            (
                "kernel32.dll".to_owned(),
                GuestAddress(KERNEL32_MODULE_HANDLE),
            ),
            ("user32.dll".to_owned(), GuestAddress(USER32_MODULE_HANDLE)),
        ]);
        let process_io = ProcessIo::sandboxed(config.filesystem_root, &config.current_directory);
        let environment = config
            .environment
            .into_iter()
            .map(|(name, value)| (name.to_ascii_uppercase(), value))
            .collect();
        Ok(Self {
            cpu,
            memory,
            api_registry,
            import_thunks,
            exit_code: None,
            started_at: Instant::now(),
            image_base: GuestAddress(image.optional.image_base),
            resource_directory,
            heaps: GuestHeapManager::new(),
            global_allocations: BTreeMap::new(),
            tls_slots: TlsSlotManager::new(image.tls.is_some()),
            unhandled_exception_filter: 0,
            mutexes: MutexManager::new(),
            events: EventManager::new(),
            com_initialization_count: 0,
            cursor_display_count: 0,
            focused_window: 0,
            virtual_memory: GuestRegionAllocator::new(GUEST_VIRTUAL_BASE, GUEST_VIRTUAL_LIMIT),
            recent_host_calls: VecDeque::with_capacity(HOST_CALL_HISTORY_LIMIT),
            host_modules,
            command_line_ansi: process_data.command_line_ansi,
            command_line_utf16: process_data.command_line_utf16,
            module_path: config.module_path,
            last_error: 0,
            process_io,
            guest_stdout: Vec::new(),
            guest_stderr: Vec::new(),
            environment,
            current_directory: config.current_directory,
            environment_block_ansi: process_data.environment_ansi,
            environment_block_utf16: process_data.environment_utf16,
            main_entry_point,
            pending_tls_callbacks,
            tls_callbacks_active,
            suspended_host_calls: Vec::new(),
            guest_callback_targets: HashMap::new(),
        })
    }

    /// Associate a guest import thunk with a registered API name.
    pub fn register_import_thunk(&mut self, address: GuestAddress, api: ApiKey) {
        self.import_thunks.insert(address, api);
    }

    /// Capture the current machine state without mutating guest execution.
    #[must_use]
    pub fn diagnostic_snapshot(&self) -> DiagnosticSnapshot {
        let mut bytes = [0; vnrt_x86::MAX_INSTRUCTION_LEN];
        let instruction_bytes = if self
            .memory
            .fetch(GuestAddress(self.cpu.state.registers.eip), &mut bytes)
            .is_ok()
        {
            bytes.to_vec()
        } else {
            Vec::new()
        };
        let stack_words = (0..9_u32)
            .map_while(|index| {
                self.cpu
                    .state
                    .registers
                    .esp
                    .checked_add(index * 4)
                    .and_then(|address| self.memory.read_u32(GuestAddress(address)).ok())
            })
            .collect();
        DiagnosticSnapshot {
            registers: self.cpu.state.registers,
            fs_base: self.cpu.state.fs_base,
            instruction_bytes,
            stack_words,
            recent_host_calls: self.recent_host_calls.iter().cloned().collect(),
        }
    }

    /// Bytes written by the guest to its standard output handle.
    #[must_use]
    pub fn guest_stdout(&self) -> &[u8] {
        &self.guest_stdout
    }

    /// Bytes written by the guest to its standard error handle.
    #[must_use]
    pub fn guest_stderr(&self) -> &[u8] {
        &self.guest_stderr
    }

    /// Run until termination or an explicit execution limit.
    pub fn run(&mut self, limits: RunLimits) -> Result<RunOutcome, RuntimeError> {
        for step_index in 0..limits.max_instructions {
            trace!(step_index, eip = self.cpu.state.registers.eip, "guest step");
            if self.tls_callbacks_active
                && self.cpu.state.registers.eip == TLS_CALLBACK_RETURN_ADDRESS
            {
                self.advance_tls_callbacks()?;
                continue;
            }
            if self.cpu.state.registers.eip == HOST_CALLBACK_RETURN_ADDRESS {
                self.advance_host_callbacks()?;
                continue;
            }
            let resolver = ThunkResolver(&self.import_thunks);
            match self.cpu.step(&mut self.memory, &resolver)? {
                StepOutcome::Continue { instruction } => {
                    trace!(?instruction, "executed instruction");
                }
                StepOutcome::ExternalCall { address } => self.dispatch_host_call(address)?,
                StepOutcome::Halted => return Ok(RunOutcome::Halted),
            }
            if let Some(code) = self.exit_code {
                return Ok(RunOutcome::Exited(code));
            }
        }
        Err(RuntimeError::ExecutionLimit(limits.max_instructions))
    }

    fn advance_tls_callbacks(&mut self) -> Result<(), RuntimeError> {
        if self.cpu.state.registers.esp != GUEST_STACK_TOP {
            return Err(RuntimeError::Unsupported(
                "TLS callback did not restore its stdcall stack frame",
            ));
        }
        if let Some(callback) = self.pending_tls_callbacks.pop_front() {
            enter_tls_callback(&mut self.cpu, &mut self.memory, callback, self.image_base)?;
        } else {
            self.tls_callbacks_active = false;
            self.cpu.state.registers.eip = self.main_entry_point.0;
        }
        Ok(())
    }

    fn advance_host_callbacks(&mut self) -> Result<(), RuntimeError> {
        let callbacks_empty = {
            let continuation =
                self.suspended_host_calls
                    .last_mut()
                    .ok_or(RuntimeError::Unsupported(
                        "Guest reached Host callback sentinel without a continuation",
                    ))?;
            if self.cpu.state.registers.esp != continuation.callback_stack_pointer {
                return Err(RuntimeError::Unsupported(
                    "Guest callback did not restore its stdcall stack frame",
                ));
            }
            if continuation.capture_callback_return {
                continuation.return_value = self.cpu.state.registers.eax;
            }
            continuation.callbacks.is_empty()
        };
        if callbacks_empty {
            let continuation = self
                .suspended_host_calls
                .pop()
                .expect("continuation was checked above");
            debug!(
                return_address = continuation.return_address,
                return_value = continuation.return_value,
                "resuming suspended Host call"
            );
            self.cpu.state.registers.eax = continuation.return_value;
            self.cpu.state.registers.esp = continuation.resumed_stack_pointer;
            self.cpu.state.registers.eip = continuation.return_address;
        } else {
            self.enter_next_host_callback()?;
        }
        Ok(())
    }

    fn enter_next_host_callback(&mut self) -> Result<(), RuntimeError> {
        let continuation =
            self.suspended_host_calls
                .last_mut()
                .ok_or(RuntimeError::Unsupported(
                    "missing Host callback continuation",
                ))?;
        let callback = continuation
            .callbacks
            .pop_front()
            .ok_or(RuntimeError::Unsupported("missing queued Guest callback"))?;
        debug!(
            target = callback.target.0,
            arguments = ?callback.arguments,
            remaining = continuation.callbacks.len(),
            "entering Guest callback"
        );
        enter_stdcall_callback(
            &mut self.cpu,
            &mut self.memory,
            callback.target,
            &callback.arguments,
            continuation.callback_stack_pointer,
        )
    }

    fn dispatch_host_call(&mut self, address: GuestAddress) -> Result<(), RuntimeError> {
        let key = self
            .import_thunks
            .get(&address)
            .cloned()
            .ok_or(RuntimeError::Unsupported("unmapped external call target"))?;
        if self.recent_host_calls.len() == HOST_CALL_HISTORY_LIMIT {
            self.recent_host_calls.pop_front();
        }
        self.recent_host_calls.push_back(key.clone());
        debug!(module = %key.module, api = %key.name, "host call");
        let handler =
            self.api_registry
                .resolve(&key)
                .ok_or_else(|| Win32Error::ApiNotRegistered {
                    module: key.module.clone(),
                    name: key.name.clone(),
                })?;

        // Guest code can access LastError directly through fs:[34h]. Refresh
        // the Host-side slot at every API boundary and publish API changes
        // back into the TEB before returning to x86 execution.
        self.last_error = self
            .memory
            .read_u32(GuestAddress(GUEST_TEB_BASE + TEB_LAST_ERROR_OFFSET))?;

        let mut context = RuntimeHostContext {
            cpu: &mut self.cpu,
            memory: &mut self.memory,
            exit_code: &mut self.exit_code,
            stdcall_cleanup: 0,
            started_at: self.started_at,
            heaps: &mut self.heaps,
            global_allocations: &mut self.global_allocations,
            tls_slots: &mut self.tls_slots,
            unhandled_exception_filter: &mut self.unhandled_exception_filter,
            mutexes: &mut self.mutexes,
            events: &mut self.events,
            com_initialization_count: &mut self.com_initialization_count,
            cursor_display_count: &mut self.cursor_display_count,
            focused_window: &mut self.focused_window,
            image_base: self.image_base,
            resource_directory: self.resource_directory,
            virtual_memory: &mut self.virtual_memory,
            api_registry: &self.api_registry,
            import_thunks: &mut self.import_thunks,
            host_modules: &self.host_modules,
            command_line_ansi: self.command_line_ansi,
            command_line_utf16: self.command_line_utf16,
            module_path: &self.module_path,
            last_error: &mut self.last_error,
            process_io: &mut self.process_io,
            guest_stdout: &mut self.guest_stdout,
            guest_stderr: &mut self.guest_stderr,
            environment: &self.environment,
            current_directory: &mut self.current_directory,
            environment_block_ansi: self.environment_block_ansi,
            environment_block_utf16: self.environment_block_utf16,
            guest_callbacks: VecDeque::new(),
            suspended_host_calls: &mut self.suspended_host_calls,
            guest_callback_targets: &mut self.guest_callback_targets,
            capture_callback_return: false,
        };
        let invocation = handler.invoke(&mut context);
        context.memory.write_u32(
            GuestAddress(GUEST_TEB_BASE + TEB_LAST_ERROR_OFFSET),
            *context.last_error,
        )?;
        invocation?;
        if context.exit_code.is_none() {
            if context.guest_callbacks.is_empty() {
                context.finish_host_return()?;
            } else {
                let stack = context.cpu.state.registers.esp;
                let return_address = context.memory.read_u32(GuestAddress(stack))?;
                let resumed_stack_pointer = stack
                    .checked_add(4)
                    .and_then(|value| value.checked_add(context.stdcall_cleanup))
                    .ok_or(RuntimeError::Unsupported("stdcall stack pointer overflow"))?;
                let return_value = context.cpu.state.registers.eax;
                let callbacks = std::mem::take(&mut context.guest_callbacks);
                let capture_callback_return = context.capture_callback_return;
                drop(context);
                self.suspended_host_calls.push(SuspendedHostCall {
                    return_address,
                    resumed_stack_pointer,
                    callback_stack_pointer: stack,
                    return_value,
                    capture_callback_return,
                    callbacks,
                });
                self.enter_next_host_callback()?;
            }
        }
        Ok(())
    }
}

struct ThunkResolver<'a>(&'a HashMap<GuestAddress, ApiKey>);

impl ExternalTargetResolver for ThunkResolver<'_> {
    fn is_external_target(&self, address: GuestAddress) -> bool {
        self.0.contains_key(&address)
    }
}
