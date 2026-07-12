//! Composition layer for PE loading, x86 stepping, memory, and host calls.

mod allocation;
mod exceptions;
mod host_context;
mod loader;
mod objects;
mod process;
mod threads;

use allocation::*;
use exceptions::*;
use host_context::*;
use loader::*;
use objects::*;
use process::*;
use threads::*;

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, trace};
use vnrt_gfx::{GraphicsDevice, TextureDescriptor, TextureId};
use vnrt_memory::{GuestAddress, GuestMemory, MemoryError, PAGE_SIZE_U32, Permissions};
use vnrt_pe::{Import, PeError, PeImage, Section};
use vnrt_win32::{
    ApiKey, ApiRegistry, FileEntry, Handle, HostCallContext, PROCESS_HEAP_HANDLE, ProcessIo,
    Win32Error,
};
use vnrt_x86::{
    BatchOutcome, ControlTransfer, CpuError, CpuException, ExternalTargetResolver, Interpreter,
    Registers, StepOutcome,
};

/// External targets occupy a reserved, unmapped region near the top of the
/// 32-bit address space. Reaching one is intercepted before instruction fetch.
const HOST_THUNK_BASE: u32 = 0xfffe_0000;
const HOST_THUNK_REGION_SIZE: u32 = 0x0001_0000;
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
const NTDLL_MODULE_HANDLE: u32 = 0x7f30_0000;
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

/// Latest normalized RGBA frame presented to one Guest window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFrame {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Top-down tightly packed RGBA8 pixels.
    pub rgba: Vec<u8>,
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
    /// A Guest window presented its first normalized frame.
    FramePresented(u32),
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
    /// Active 32-bit SEH registration records as `(record, handler)` pairs.
    pub exception_chain: Vec<(u32, u32)>,
    /// Most recent host calls, oldest first.
    pub recent_host_calls: Vec<ApiKey>,
    /// Recent indirect calls, jumps, and returns, oldest first.
    pub recent_control_transfers: Vec<ControlTransfer>,
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
    /// A second processor fault occurred while a Guest SEH dispatch was active.
    #[error(
        "nested processor exception during SEH dispatch (pending {pending_code:#010x} at {pending_address:#010x}, eip={eip:#010x}): {nested}"
    )]
    UnsupportedNestedException {
        /// Exception code of the still-active SEH dispatch.
        pending_code: u32,
        /// Fault address recorded for the still-active SEH dispatch.
        pending_address: u32,
        /// Debug representation of the nested processor exception.
        nested: String,
        /// Guest EIP when the nested fault was observed.
        eip: u32,
    },
    /// No Guest SEH frame accepted a synchronous processor exception.
    #[error("unhandled Guest exception {code:#010x} at {address:#010x}")]
    UnhandledGuestException {
        /// Win32 exception status code.
        code: u32,
        /// Address that raised the exception.
        address: u32,
    },
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
    tokens: TokenManager,
    threads: ThreadManager,
    com_initialization_count: u32,
    cursor_display_count: i32,
    focused_window: u32,
    window_class_longs: HashMap<(u32, i32), u32>,
    icons: BTreeSet<u32>,
    next_icon_handle: u32,
    window_classes: HashMap<String, (u16, GuestAddress)>,
    next_window_class_atom: u16,
    window_regions: HashMap<u32, u32>,
    windows: HashMap<u32, String>,
    window_titles: HashMap<u32, String>,
    visible_windows: BTreeSet<u32>,
    window_placements: HashMap<u32, Vec<u8>>,
    disabled_windows: BTreeSet<u32>,
    thread_messages: VecDeque<(u32, u32, u32, u32)>,
    primary_display_size: (u32, u32),
    menus: BTreeSet<u32>,
    next_menu_handle: u32,
    menu_children: HashMap<u32, Vec<u32>>,
    cursor_position: (i32, i32),
    window_menus: HashMap<u32, u32>,
    clipboard_open: bool,
    clipboard_data: HashMap<u32, u32>,
    window_longs: HashMap<(u32, i32), u32>,
    invalidated_windows: BTreeSet<u32>,
    window_dcs: HashMap<u32, u32>,
    next_window_dc: u32,
    keyboard_state: [u8; 256],
    memory_dcs: BTreeSet<u32>,
    next_memory_dc: u32,
    selected_gdi_objects: HashMap<u32, u32>,
    gdi_objects: HashMap<u32, Vec<u8>>,
    next_gdi_object: u32,
    gdi_dc_attributes: HashMap<(u32, u32), u32>,
    window_frames: HashMap<u32, WindowFrame>,
    graphics: Option<Box<dyn GraphicsDevice>>,
    next_window_handle: u32,
    virtual_memory: GuestRegionAllocator,
    recent_host_calls: VecDeque<ApiKey>,
    host_modules: HashMap<String, GuestAddress>,
    command_line_ansi: GuestAddress,
    command_line_utf16: GuestAddress,
    process_parameters: GuestAddress,
    module_path: String,
    last_error: u32,
    error_mode: u32,
    process_io: ProcessIo,
    guest_stdout: Vec<u8>,
    guest_stderr: Vec<u8>,
    standard_handles: [u32; 3],
    environment: BTreeMap<String, String>,
    current_directory: String,
    environment_block_ansi: GuestAddress,
    environment_block_utf16: GuestAddress,
    main_entry_point: GuestAddress,
    pending_tls_callbacks: VecDeque<GuestAddress>,
    tls_callbacks_active: bool,
    suspended_host_calls: Vec<SuspendedHostCall>,
    guest_callback_targets: HashMap<u32, GuestAddress>,
    pending_exception: Option<PendingException>,
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
        initialize_host_thunk_region(&mut memory)?;
        let import_thunks = bind_imports(&mut memory, &image)?;
        let mut import_thunks = import_thunks;
        let host_modules = initialize_host_modules(&mut memory, &api_registry, &mut import_thunks)?;
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
            enter_main_entry_point(&mut cpu, &mut memory, main_entry_point)?;
            false
        };
        let process_io = ProcessIo::sandboxed(config.filesystem_root, &config.current_directory);
        let environment = config
            .environment
            .into_iter()
            .map(|(name, value)| (name.to_ascii_uppercase(), value))
            .collect();
        let threads = ThreadManager::new_main(&cpu.state);
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
            tokens: TokenManager::new(),
            threads,
            com_initialization_count: 0,
            cursor_display_count: 0,
            focused_window: 0,
            window_class_longs: HashMap::new(),
            icons: BTreeSet::new(),
            next_icon_handle: 0x0007_0000,
            window_classes: HashMap::new(),
            next_window_class_atom: 0xc000,
            window_regions: HashMap::new(),
            windows: HashMap::new(),
            window_titles: HashMap::new(),
            visible_windows: BTreeSet::new(),
            window_placements: HashMap::new(),
            disabled_windows: BTreeSet::new(),
            thread_messages: VecDeque::new(),
            primary_display_size: (1280, 720),
            menus: BTreeSet::new(),
            next_menu_handle: 0x0009_0000,
            menu_children: HashMap::new(),
            cursor_position: (640, 360),
            window_menus: HashMap::new(),
            clipboard_open: false,
            clipboard_data: HashMap::new(),
            window_longs: HashMap::new(),
            invalidated_windows: BTreeSet::new(),
            window_dcs: HashMap::new(),
            next_window_dc: 0x000a_0000,
            keyboard_state: [0; 256],
            memory_dcs: BTreeSet::new(),
            next_memory_dc: 0x000b_0000,
            selected_gdi_objects: HashMap::new(),
            gdi_objects: HashMap::new(),
            next_gdi_object: 0x000d_0000,
            gdi_dc_attributes: HashMap::new(),
            window_frames: HashMap::new(),
            graphics: None,
            next_window_handle: 0x0008_0000,
            virtual_memory: GuestRegionAllocator::new(GUEST_VIRTUAL_BASE, GUEST_VIRTUAL_LIMIT),
            recent_host_calls: VecDeque::with_capacity(HOST_CALL_HISTORY_LIMIT),
            host_modules,
            command_line_ansi: process_data.command_line_ansi,
            command_line_utf16: process_data.command_line_utf16,
            process_parameters: process_data.process_parameters,
            module_path: config.module_path,
            last_error: 0,
            error_mode: 0,
            process_io,
            guest_stdout: Vec::new(),
            guest_stderr: Vec::new(),
            standard_handles: [
                STD_INPUT_HANDLE_VALUE,
                STD_OUTPUT_HANDLE_VALUE,
                STD_ERROR_HANDLE_VALUE,
            ],
            environment,
            current_directory: config.current_directory,
            environment_block_ansi: process_data.environment_ansi,
            environment_block_utf16: process_data.environment_utf16,
            main_entry_point,
            pending_tls_callbacks,
            tls_callbacks_active,
            suspended_host_calls: Vec::new(),
            guest_callback_targets: HashMap::new(),
            pending_exception: None,
        })
    }

    /// Associate a guest import thunk with a registered API name.
    pub fn register_import_thunk(&mut self, address: GuestAddress, api: ApiKey) {
        self.import_thunks.insert(address, api);
    }

    /// Look up one synthetic Host module for loader/debugger diagnostics.
    #[must_use]
    pub fn host_module_handle(&self, name: &str) -> Option<GuestAddress> {
        self.host_modules.get(&name.to_ascii_lowercase()).copied()
    }

    /// Identify the Host API dispatched by one executable thunk address.
    #[must_use]
    pub fn host_api_at(&self, address: GuestAddress) -> Option<&ApiKey> {
        self.import_thunks.get(&address)
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
        let mut exception_chain = Vec::new();
        let mut record = self
            .memory
            .read_u32(GuestAddress(self.cpu.state.fs_base))
            .unwrap_or(u32::MAX);
        while record != u32::MAX && exception_chain.len() < 16 {
            let Ok(next) = self.memory.read_u32(GuestAddress(record)) else {
                break;
            };
            let Ok(handler) = self.memory.read_u32(GuestAddress(record.wrapping_add(4))) else {
                break;
            };
            exception_chain.push((record, handler));
            if next == record {
                break;
            }
            record = next;
        }
        DiagnosticSnapshot {
            registers: self.cpu.state.registers,
            fs_base: self.cpu.state.fs_base,
            instruction_bytes,
            stack_words,
            exception_chain,
            recent_host_calls: self.recent_host_calls.iter().cloned().collect(),
            recent_control_transfers: self.cpu.recent_control_transfers(),
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

    /// Latest normalized frame presented to a Guest window, if any.
    #[must_use]
    pub fn window_frame(&self, window: u32) -> Option<&WindowFrame> {
        self.window_frames.get(&window)
    }

    /// Snapshot the handles of windows that have presented at least one frame.
    #[must_use]
    pub fn presented_window_handles(&self) -> Vec<u32> {
        let mut windows = self.window_frames.keys().copied().collect::<Vec<_>>();
        windows.sort_unstable();
        windows
    }

    /// Attach the real Host GPU used by Direct3D compatibility frontends.
    pub fn set_graphics_device(&mut self, graphics: Box<dyn GraphicsDevice>) {
        self.graphics = Some(graphics);
    }

    /// Run until termination or an explicit execution limit.
    pub fn run(&mut self, limits: RunLimits) -> Result<RunOutcome, RuntimeError> {
        if tracing::enabled!(target: "vnrt_runtime", tracing::Level::TRACE) {
            return self.run_traced(limits, false);
        }
        self.run_batched(limits, false)
    }

    /// Run until the first Guest frame, termination, or the execution limit.
    pub fn run_until_first_frame(&mut self, limits: RunLimits) -> Result<RunOutcome, RuntimeError> {
        if let Some(window) = self.window_frames.keys().copied().min() {
            return Ok(RunOutcome::FramePresented(window));
        }
        if tracing::enabled!(target: "vnrt_runtime", tracing::Level::TRACE) {
            return self.run_traced(limits, true);
        }
        self.run_batched(limits, true)
    }

    fn run_batched(
        &mut self,
        limits: RunLimits,
        stop_at_first_frame: bool,
    ) -> Result<RunOutcome, RuntimeError> {
        let mut consumed = 0;
        while consumed < limits.max_instructions {
            if self.tls_callbacks_active
                && self.cpu.state.registers.eip == TLS_CALLBACK_RETURN_ADDRESS
            {
                self.advance_tls_callbacks()?;
                consumed += 1;
                continue;
            }
            if self.cpu.state.registers.eip == HOST_CALLBACK_RETURN_ADDRESS {
                self.advance_host_callbacks()?;
                consumed += 1;
                continue;
            }
            if self.cpu.state.registers.eip == EXCEPTION_HANDLER_RETURN_ADDRESS {
                self.advance_exception_dispatch()?;
                consumed += 1;
                continue;
            }
            if self.cpu.state.registers.eip == THREAD_EXIT_RETURN_ADDRESS {
                self.finish_thread_procedure_return()?;
                consumed += 1;
                continue;
            }
            let resolver = ThunkResolver(&self.import_thunks);
            match self.cpu.run_batch(
                &mut self.memory,
                &resolver,
                limits.max_instructions - consumed,
            )? {
                BatchOutcome::BudgetExhausted { steps } => consumed += steps,
                BatchOutcome::ExternalCall { address, steps } => {
                    consumed += steps;
                    if self.tls_callbacks_active && address.0 == TLS_CALLBACK_RETURN_ADDRESS {
                        self.advance_tls_callbacks()?;
                    } else if address.0 == HOST_CALLBACK_RETURN_ADDRESS {
                        self.advance_host_callbacks()?;
                    } else if address.0 == EXCEPTION_HANDLER_RETURN_ADDRESS {
                        self.advance_exception_dispatch()?;
                    } else if address.0 == THREAD_EXIT_RETURN_ADDRESS {
                        self.finish_thread_procedure_return()?;
                    } else {
                        self.dispatch_host_call(address)?;
                    }
                }
                BatchOutcome::Exception { exception, steps } => {
                    consumed += steps;
                    self.dispatch_cpu_exception(exception)?;
                }
                BatchOutcome::Halted { .. } => return Ok(RunOutcome::Halted),
            }
            if let Some(code) = self.exit_code {
                return Ok(RunOutcome::Exited(code));
            }
            if stop_at_first_frame && let Some(window) = self.window_frames.keys().copied().min() {
                return Ok(RunOutcome::FramePresented(window));
            }
        }
        Err(RuntimeError::ExecutionLimit(limits.max_instructions))
    }

    fn run_traced(
        &mut self,
        limits: RunLimits,
        stop_at_first_frame: bool,
    ) -> Result<RunOutcome, RuntimeError> {
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
            if self.cpu.state.registers.eip == EXCEPTION_HANDLER_RETURN_ADDRESS {
                self.advance_exception_dispatch()?;
                continue;
            }
            if self.cpu.state.registers.eip == THREAD_EXIT_RETURN_ADDRESS {
                self.finish_thread_procedure_return()?;
                continue;
            }
            let resolver = ThunkResolver(&self.import_thunks);
            match self.cpu.step(&mut self.memory, &resolver)? {
                StepOutcome::Continue { instruction } => {
                    trace!(?instruction, "executed instruction");
                }
                StepOutcome::ExternalCall { address } => {
                    if address.0 == THREAD_EXIT_RETURN_ADDRESS {
                        self.finish_thread_procedure_return()?;
                    } else {
                        self.dispatch_host_call(address)?;
                    }
                }
                StepOutcome::Exception { exception } => {
                    self.dispatch_cpu_exception(exception)?;
                }
                StepOutcome::Halted => return Ok(RunOutcome::Halted),
            }
            if let Some(code) = self.exit_code {
                return Ok(RunOutcome::Exited(code));
            }
            if stop_at_first_frame && let Some(window) = self.window_frames.keys().copied().min() {
                return Ok(RunOutcome::FramePresented(window));
            }
        }
        Err(RuntimeError::ExecutionLimit(limits.max_instructions))
    }

    fn finish_thread_procedure_return(&mut self) -> Result<(), RuntimeError> {
        let exit_code = self.cpu.state.registers.eax;
        self.exit_guest_thread_internal(exit_code)
    }

    fn exit_guest_thread_internal(&mut self, exit_code: u32) -> Result<(), RuntimeError> {
        if !self.suspended_host_calls.is_empty() {
            return Err(RuntimeError::Unsupported(
                "ExitThread with suspended Host callbacks",
            ));
        }
        let is_main = self.threads.exit_current(exit_code);
        self.wake_waiters_after_thread_exit()?;
        if is_main {
            self.exit_code = Some(exit_code);
            self.cpu.state.halted = true;
            return Ok(());
        }
        let next = self
            .threads
            .pick_runnable_any()
            .ok_or(RuntimeError::Unsupported(
                "no runnable Guest thread after ExitThread",
            ))?;
        self.threads.switch_to(
            next,
            &mut self.cpu.state,
            &mut self.last_error,
            &mut self.memory,
        )?;
        Ok(())
    }

    fn wake_waiters_after_thread_exit(&mut self) -> Result<(), RuntimeError> {
        let waiters = self.threads.waiting_threads();
        let mut completions = Vec::new();
        for (thread_id, wait) in waiters {
            if let Some(result) = try_wait_objects(
                &mut self.events,
                &mut self.mutexes,
                &self.threads,
                thread_id,
                &wait.handles,
                wait.wait_all,
                true,
            )
            .map_err(RuntimeError::from)?
            {
                completions.push((thread_id, result, wait.cleanup));
            }
        }
        for (thread_id, result, cleanup) in completions {
            self.threads
                .complete_wait(thread_id, result, cleanup)
                .map_err(RuntimeError::from)?;
        }
        Ok(())
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
            enter_main_entry_point(&mut self.cpu, &mut self.memory, self.main_entry_point)?;
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
        // back into the current thread's TEB before returning to x86 execution.
        let teb_base = self.threads.current_teb();
        self.last_error = self
            .memory
            .read_u32(GuestAddress(teb_base + TEB_LAST_ERROR_OFFSET))?;

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
            tokens: &mut self.tokens,
            threads: &mut self.threads,
            scheduler_switched: false,
            com_initialization_count: &mut self.com_initialization_count,
            cursor_display_count: &mut self.cursor_display_count,
            focused_window: &mut self.focused_window,
            window_class_longs: &mut self.window_class_longs,
            icons: &mut self.icons,
            next_icon_handle: &mut self.next_icon_handle,
            window_classes: &mut self.window_classes,
            next_window_class_atom: &mut self.next_window_class_atom,
            window_regions: &mut self.window_regions,
            windows: &mut self.windows,
            window_titles: &mut self.window_titles,
            visible_windows: &mut self.visible_windows,
            window_placements: &mut self.window_placements,
            disabled_windows: &mut self.disabled_windows,
            thread_messages: &mut self.thread_messages,
            primary_display_size: &mut self.primary_display_size,
            menus: &mut self.menus,
            next_menu_handle: &mut self.next_menu_handle,
            menu_children: &mut self.menu_children,
            cursor_position: &mut self.cursor_position,
            window_menus: &mut self.window_menus,
            clipboard_open: &mut self.clipboard_open,
            clipboard_data: &mut self.clipboard_data,
            window_longs: &mut self.window_longs,
            invalidated_windows: &mut self.invalidated_windows,
            window_dcs: &mut self.window_dcs,
            next_window_dc: &mut self.next_window_dc,
            keyboard_state: &mut self.keyboard_state,
            memory_dcs: &mut self.memory_dcs,
            next_memory_dc: &mut self.next_memory_dc,
            selected_gdi_objects: &mut self.selected_gdi_objects,
            gdi_objects: &mut self.gdi_objects,
            next_gdi_object: &mut self.next_gdi_object,
            gdi_dc_attributes: &mut self.gdi_dc_attributes,
            window_frames: &mut self.window_frames,
            graphics: &mut self.graphics,
            next_window_handle: &mut self.next_window_handle,
            image_base: self.image_base,
            resource_directory: self.resource_directory,
            virtual_memory: &mut self.virtual_memory,
            api_registry: &self.api_registry,
            import_thunks: &mut self.import_thunks,
            host_modules: &self.host_modules,
            command_line_ansi: self.command_line_ansi,
            command_line_utf16: self.command_line_utf16,
            process_parameters: self.process_parameters,
            module_path: &self.module_path,
            last_error: &mut self.last_error,
            error_mode: &mut self.error_mode,
            process_io: &mut self.process_io,
            guest_stdout: &mut self.guest_stdout,
            guest_stderr: &mut self.guest_stderr,
            standard_handles: &mut self.standard_handles,
            environment: &mut self.environment,
            current_directory: &mut self.current_directory,
            environment_block_ansi: &mut self.environment_block_ansi,
            environment_block_utf16: &mut self.environment_block_utf16,
            guest_callbacks: VecDeque::new(),
            suspended_host_calls: &mut self.suspended_host_calls,
            guest_callback_targets: &mut self.guest_callback_targets,
            capture_callback_return: false,
            raised_exception: None,
        };
        let invocation = handler.invoke(&mut context);
        let teb_base = context.threads.current_teb();
        context.memory.write_u32(
            GuestAddress(teb_base + TEB_LAST_ERROR_OFFSET),
            *context.last_error,
        )?;
        let scheduler_switched = context.scheduler_switched;
        invocation?;
        if scheduler_switched {
            // The cooperative scheduler already installed another Guest context.
            return Ok(());
        }
        if context.exit_code.is_none() {
            if context.guest_callbacks.is_empty() {
                context.finish_host_return()?;
                let raised_exception = context.raised_exception.take();
                drop(context);
                if let Some((code, flags, information)) = raised_exception {
                    let address = GuestAddress(self.cpu.state.registers.eip);
                    self.dispatch_guest_exception(code, flags, address, &information)?;
                }
            } else {
                if context.raised_exception.is_some() {
                    return Err(RuntimeError::Unsupported(
                        "Host call requested callbacks and an exception",
                    ));
                }
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
            || matches!(
                address.0,
                TLS_CALLBACK_RETURN_ADDRESS
                    | HOST_CALLBACK_RETURN_ADDRESS
                    | EXCEPTION_HANDLER_RETURN_ADDRESS
                    | THREAD_EXIT_RETURN_ADDRESS
            )
    }
}
