use super::*;

pub(super) struct RuntimeHostContext<'a> {
    pub(super) cpu: &'a mut Interpreter,
    pub(super) memory: &'a mut GuestMemory,
    pub(super) exit_code: &'a mut Option<u32>,
    pub(super) stdcall_cleanup: u32,
    pub(super) started_at: Instant,
    pub(super) heaps: &'a mut GuestHeapManager,
    pub(super) global_allocations: &'a mut BTreeMap<u32, u32>,
    pub(super) tls_slots: &'a mut TlsSlotManager,
    pub(super) unhandled_exception_filter: &'a mut u32,
    pub(super) mutexes: &'a mut MutexManager,
    pub(super) events: &'a mut EventManager,
    pub(super) tokens: &'a mut TokenManager,
    pub(super) com_initialization_count: &'a mut u32,
    pub(super) cursor_display_count: &'a mut i32,
    pub(super) focused_window: &'a mut u32,
    pub(super) image_base: GuestAddress,
    pub(super) resource_directory: Option<(GuestAddress, u32)>,
    pub(super) virtual_memory: &'a mut GuestRegionAllocator,
    pub(super) api_registry: &'a ApiRegistry,
    pub(super) import_thunks: &'a mut HashMap<GuestAddress, ApiKey>,
    pub(super) host_modules: &'a HashMap<String, GuestAddress>,
    pub(super) command_line_ansi: GuestAddress,
    pub(super) command_line_utf16: GuestAddress,
    pub(super) module_path: &'a str,
    pub(super) last_error: &'a mut u32,
    pub(super) process_io: &'a mut ProcessIo,
    pub(super) guest_stdout: &'a mut Vec<u8>,
    pub(super) guest_stderr: &'a mut Vec<u8>,
    pub(super) environment: &'a BTreeMap<String, String>,
    pub(super) current_directory: &'a mut String,
    pub(super) environment_block_ansi: GuestAddress,
    pub(super) environment_block_utf16: GuestAddress,
    pub(super) guest_callbacks: VecDeque<GuestCallback>,
    pub(super) suspended_host_calls: &'a mut Vec<SuspendedHostCall>,
    pub(super) guest_callback_targets: &'a mut HashMap<u32, GuestAddress>,
    pub(super) capture_callback_return: bool,
}

impl RuntimeHostContext<'_> {
    pub(super) fn finish_host_return(&mut self) -> Result<(), RuntimeError> {
        let stack = self.cpu.state.registers.esp;
        let return_address = self.memory.read_u32(GuestAddress(stack))?;
        self.cpu.state.registers.esp = stack
            .checked_add(4)
            .and_then(|value| value.checked_add(self.stdcall_cleanup))
            .ok_or(RuntimeError::Unsupported("stdcall stack pointer overflow"))?;
        self.cpu.state.registers.eip = return_address;
        Ok(())
    }
}

impl HostCallContext for RuntimeHostContext<'_> {
    fn argument_u32(&self, index: usize) -> Result<u32, Win32Error> {
        let byte_offset = u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .and_then(|value| value.checked_add(4))
            .ok_or(Win32Error::InvalidArgument(
                "stdcall argument index overflow",
            ))?;
        let address = self
            .cpu
            .state
            .registers
            .esp
            .checked_add(byte_offset)
            .ok_or(Win32Error::InvalidArgument(
                "stdcall argument address overflow",
            ))?;
        self.memory
            .read_u32(GuestAddress(address))
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))
    }

    fn set_return_u32(&mut self, value: u32) {
        self.cpu.state.registers.eax = value;
    }

    fn set_stdcall_cleanup(&mut self, argument_bytes: u32) {
        self.stdcall_cleanup = argument_bytes;
    }

    fn request_guest_callback(
        &mut self,
        callback: GuestAddress,
        arguments: &[u32],
    ) -> Result<(), Win32Error> {
        if callback.0 == 0 {
            return Err(Win32Error::InvalidArgument("null Guest callback"));
        }
        if arguments.len() > MAX_HOST_CALLBACK_ARGUMENTS {
            return Err(Win32Error::InvalidArgument(
                "Guest callback argument limit exceeded",
            ));
        }
        self.guest_callbacks.push_back(GuestCallback {
            target: callback,
            arguments: arguments.to_vec(),
        });
        Ok(())
    }

    fn use_guest_callback_return_value(&mut self) {
        self.capture_callback_return = true;
    }

    fn complete_suspended_host_call(&mut self, return_value: u32) -> Result<(), Win32Error> {
        let continuation =
            self.suspended_host_calls
                .last_mut()
                .ok_or(Win32Error::InvalidArgument(
                    "no suspended Host call for Guest callback",
                ))?;
        continuation.return_value = return_value;
        continuation.callbacks.clear();
        Ok(())
    }

    fn register_guest_callback_target(&mut self, object: u32, callback: GuestAddress) {
        self.guest_callback_targets.insert(object, callback);
    }

    fn guest_callback_target(&self, object: u32) -> Option<GuestAddress> {
        self.guest_callback_targets.get(&object).copied()
    }

    fn replace_focus_window(&mut self, window: u32) -> u32 {
        std::mem::replace(self.focused_window, window)
    }

    fn read_memory(&self, address: GuestAddress, output: &mut [u8]) -> Result<(), Win32Error> {
        self.memory
            .read(address, output)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))
    }

    fn write_memory(&mut self, address: GuestAddress, bytes: &[u8]) -> Result<(), Win32Error> {
        self.memory
            .write(address, bytes)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))
    }

    fn request_exit(&mut self, code: u32) {
        *self.exit_code = Some(code);
        self.cpu.state.halted = true;
    }

    fn tick_count(&self) -> u32 {
        self.started_at.elapsed().as_millis() as u32
    }

    fn performance_counter(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    fn performance_frequency(&self) -> u64 {
        1_000_000_000
    }

    fn system_time_filetime(&self) -> u64 {
        const WINDOWS_TO_UNIX_SECONDS: u64 = 11_644_473_600;
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        WINDOWS_TO_UNIX_SECONDS
            .saturating_add(unix.as_secs())
            .saturating_mul(10_000_000)
            .saturating_add(u64::from(unix.subsec_nanos() / 100))
    }

    fn create_heap(
        &mut self,
        initial_size: u32,
        maximum_size: u32,
        executable: bool,
    ) -> Result<Handle, Win32Error> {
        self.heaps.create(initial_size, maximum_size, executable)
    }

    fn destroy_heap(&mut self, heap: Handle) -> Result<(), Win32Error> {
        self.heaps.destroy(self.memory, heap)
    }

    fn allocate_heap_memory(
        &mut self,
        heap: Handle,
        size: u32,
    ) -> Result<GuestAddress, Win32Error> {
        self.heaps.allocate(self.memory, heap, size)
    }

    fn reallocate_heap_memory(
        &mut self,
        heap: Handle,
        address: GuestAddress,
        size: u32,
    ) -> Result<GuestAddress, Win32Error> {
        self.heaps.reallocate(self.memory, heap, address, size)
    }

    fn free_heap_memory(&mut self, heap: Handle, address: GuestAddress) -> Result<(), Win32Error> {
        self.heaps.free(self.memory, heap, address)
    }

    fn heap_memory_size(&self, heap: Handle, address: GuestAddress) -> Result<u32, Win32Error> {
        self.heaps.size(heap, address)
    }

    fn allocate_global_memory(&mut self, size: u32) -> Result<Handle, Win32Error> {
        let address = self
            .heaps
            .allocate(self.memory, Handle(PROCESS_HEAP_HANDLE), size)?;
        self.global_allocations.insert(address.0, 0);
        Ok(Handle(address.0))
    }

    fn lock_global_memory(&mut self, handle: Handle) -> Result<GuestAddress, Win32Error> {
        let lock_count = self
            .global_allocations
            .get_mut(&handle.0)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        *lock_count = lock_count.checked_add(1).ok_or(Win32Error::OutOfMemory)?;
        Ok(GuestAddress(handle.0))
    }

    fn unlock_global_memory(&mut self, handle: Handle) -> Result<bool, Win32Error> {
        let lock_count = self
            .global_allocations
            .get_mut(&handle.0)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        if *lock_count == 0 {
            return Err(Win32Error::InvalidArgument("GlobalUnlock without lock"));
        }
        *lock_count -= 1;
        Ok(*lock_count != 0)
    }

    fn free_global_memory(&mut self, handle: Handle) -> Result<(), Win32Error> {
        self.global_allocations
            .remove(&handle.0)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        self.heaps.free(
            self.memory,
            Handle(PROCESS_HEAP_HANDLE),
            GuestAddress(handle.0),
        )
    }

    fn allocate_tls_index(&mut self) -> Result<u32, Win32Error> {
        self.tls_slots.allocate(self.memory)
    }

    fn free_tls_index(&mut self, index: u32) -> Result<(), Win32Error> {
        self.tls_slots.free(self.memory, index)
    }

    fn tls_value(&self, index: u32) -> Result<u32, Win32Error> {
        self.tls_slots.get(self.memory, index)
    }

    fn set_tls_value(&mut self, index: u32, value: u32) -> Result<(), Win32Error> {
        self.tls_slots.set(self.memory, index, value)
    }

    fn replace_unhandled_exception_filter(&mut self, filter: u32) -> u32 {
        std::mem::replace(self.unhandled_exception_filter, filter)
    }

    fn unhandled_exception_filter(&self) -> GuestAddress {
        GuestAddress(*self.unhandled_exception_filter)
    }

    fn initialize_com(&mut self) -> u32 {
        let result = u32::from(*self.com_initialization_count != 0); // S_OK / S_FALSE
        *self.com_initialization_count = self.com_initialization_count.saturating_add(1);
        result
    }

    fn uninitialize_com(&mut self) {
        *self.com_initialization_count = self.com_initialization_count.saturating_sub(1);
    }

    fn adjust_cursor_display_count(&mut self, show: bool) -> i32 {
        *self.cursor_display_count = if show {
            self.cursor_display_count.saturating_add(1)
        } else {
            self.cursor_display_count.saturating_sub(1)
        };
        *self.cursor_display_count
    }

    fn main_module_base(&self) -> GuestAddress {
        self.image_base
    }

    fn resource_directory(&self) -> Option<(GuestAddress, u32)> {
        self.resource_directory
    }

    fn allocate_virtual_memory(
        &mut self,
        size: u32,
        read: bool,
        write: bool,
        execute: bool,
    ) -> Result<GuestAddress, Win32Error> {
        self.virtual_memory
            .allocate(self.memory, size, Permissions::new(read, write, execute))
    }

    fn free_virtual_memory(&mut self, address: GuestAddress) -> Result<(), Win32Error> {
        self.virtual_memory.free(self.memory, address)
    }

    fn reserve_virtual_memory(&mut self, size: u32) -> Result<GuestAddress, Win32Error> {
        self.virtual_memory.reserve(self.memory, size)
    }

    fn commit_virtual_memory(
        &mut self,
        address: GuestAddress,
        size: u32,
        read: bool,
        write: bool,
        execute: bool,
    ) -> Result<(), Win32Error> {
        self.virtual_memory.commit(
            self.memory,
            address,
            size,
            Permissions::new(read, write, execute),
        )
    }

    fn protect_virtual_memory(
        &mut self,
        address: GuestAddress,
        size: u32,
        read: bool,
        write: bool,
        execute: bool,
    ) -> Result<(bool, bool, bool), Win32Error> {
        if size == 0 {
            return Err(Win32Error::InvalidArgument("zero virtual protection size"));
        }
        let start = address.0 & !(PAGE_SIZE_U32 - 1);
        let end = address
            .0
            .checked_add(size)
            .and_then(|end| align_up(end, PAGE_SIZE_U32))
            .ok_or(Win32Error::OutOfMemory)?;
        let old = self
            .memory
            .permissions_at(GuestAddress(start))
            .ok_or(Win32Error::InvalidAllocation { address: address.0 })?;
        self.memory
            .protect_range(
                GuestAddress(start),
                end - start,
                Permissions::new(read, write, execute),
            )
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;
        Ok((old.read, old.write, old.execute))
    }

    fn loaded_module_handle(&self, name: &str) -> Option<GuestAddress> {
        let mut normalized = name.to_ascii_lowercase();
        if !normalized.contains('.') {
            normalized.push_str(".dll");
        }
        self.host_modules.get(&normalized).copied()
    }

    fn resolve_host_api(
        &mut self,
        module: GuestAddress,
        name: &str,
    ) -> Result<GuestAddress, Win32Error> {
        let module_name = self
            .host_modules
            .iter()
            .find_map(|(name, handle)| (*handle == module).then(|| name.clone()))
            .ok_or_else(|| Win32Error::ModuleNotFound(format!("{:#010x}", module.0)))?;
        let key = ApiKey::new(module_name.clone(), name);
        if self.api_registry.resolve(&key).is_none() {
            return Err(Win32Error::ProcedureNotFound {
                module: module_name,
                name: name.to_owned(),
            });
        }
        if let Some((address, _)) = self
            .import_thunks
            .iter()
            .find(|(_, existing)| **existing == key)
        {
            return Ok(*address);
        }
        let index = u32::try_from(self.import_thunks.len()).map_err(|_| Win32Error::OutOfMemory)?;
        let address = HOST_THUNK_BASE
            .checked_add(index.checked_mul(4).ok_or(Win32Error::OutOfMemory)?)
            .map(GuestAddress)
            .ok_or(Win32Error::OutOfMemory)?;
        self.import_thunks.insert(address, key);
        Ok(address)
    }

    fn command_line_ansi(&self) -> GuestAddress {
        self.command_line_ansi
    }

    fn command_line_utf16(&self) -> GuestAddress {
        self.command_line_utf16
    }

    fn main_module_path(&self) -> &str {
        self.module_path
    }

    fn last_error(&self) -> u32 {
        *self.last_error
    }

    fn set_last_error(&mut self, value: u32) {
        *self.last_error = value;
    }

    fn open_file_read(&mut self, path: &str) -> Result<Handle, Win32Error> {
        self.process_io.open_read(path)
    }

    fn read_file(&mut self, handle: Handle, length: usize) -> Result<Vec<u8>, Win32Error> {
        self.process_io.read(handle, length)
    }

    fn close_file(&mut self, handle: Handle) -> Result<(), Win32Error> {
        self.process_io.close(handle)
    }

    fn close_kernel_handle(&mut self, handle: Handle) -> Result<(), Win32Error> {
        if self.process_io.contains(handle) {
            self.process_io.close(handle)
        } else {
            self.mutexes
                .close(handle)
                .or_else(|_| self.events.close(handle))
                .or_else(|_| self.tokens.close(handle))
        }
    }

    fn open_process_token(
        &mut self,
        process: Handle,
        desired_access: u32,
    ) -> Result<Handle, Win32Error> {
        if process.0 != u32::MAX {
            return Err(Win32Error::InvalidHandle(process.0));
        }
        self.tokens.open(desired_access)
    }

    fn token_is_open(&self, token: Handle) -> bool {
        self.tokens.contains(token)
    }

    fn create_mutex(
        &mut self,
        name: Option<&str>,
        initial_owner: bool,
    ) -> Result<(Handle, bool), Win32Error> {
        self.mutexes
            .create(name, initial_owner, self.current_thread_id())
    }

    fn release_mutex(&mut self, handle: Handle) -> Result<(), Win32Error> {
        self.mutexes.release(handle, self.current_thread_id())
    }

    fn create_event(
        &mut self,
        name: Option<&str>,
        manual_reset: bool,
        initial_state: bool,
    ) -> Result<(Handle, bool), Win32Error> {
        self.events.create(name, manual_reset, initial_state)
    }

    fn set_event_state(&mut self, handle: Handle, signaled: bool) -> Result<(), Win32Error> {
        self.events.set_state(handle, signaled)
    }

    fn try_wait_for_objects(
        &mut self,
        handles: &[Handle],
        wait_all: bool,
    ) -> Result<Option<u32>, Win32Error> {
        if handles.is_empty() {
            return Err(Win32Error::InvalidArgument("empty wait handle array"));
        }
        let thread = self.current_thread_id();
        let readiness = handles
            .iter()
            .map(|handle| {
                self.events
                    .is_signaled(*handle)
                    .or_else(|| self.mutexes.is_available(*handle, thread))
                    .ok_or(Win32Error::InvalidHandle(handle.0))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if wait_all {
            if !readiness.iter().all(|ready| *ready) {
                return Ok(None);
            }
            for handle in handles {
                self.events
                    .consume(*handle)
                    .or_else(|| self.mutexes.acquire(*handle, thread));
            }
            Ok(Some(0))
        } else if let Some(index) = readiness.iter().position(|ready| *ready) {
            let handle = handles[index];
            self.events
                .consume(handle)
                .or_else(|| self.mutexes.acquire(handle, thread));
            u32::try_from(index)
                .map(Some)
                .map_err(|_| Win32Error::OutOfMemory)
        } else {
            Ok(None)
        }
    }

    fn find_first_file(&mut self, pattern: &str) -> Result<(Handle, FileEntry), Win32Error> {
        self.process_io.find_first(pattern)
    }

    fn find_next_file(&mut self, handle: Handle) -> Result<Option<FileEntry>, Win32Error> {
        self.process_io.find_next(handle)
    }

    fn close_file_search(&mut self, handle: Handle) -> Result<(), Win32Error> {
        self.process_io.close_search(handle)
    }

    fn file_size(&self, handle: Handle) -> Result<u64, Win32Error> {
        self.process_io.file_size(handle)
    }

    fn standard_handle(&self, selector: i32) -> Option<Handle> {
        match selector {
            -10 => Some(Handle(STD_INPUT_HANDLE_VALUE)),
            -11 => Some(Handle(STD_OUTPUT_HANDLE_VALUE)),
            -12 => Some(Handle(STD_ERROR_HANDLE_VALUE)),
            _ => None,
        }
    }

    fn write_handle(&mut self, handle: Handle, bytes: &[u8]) -> Result<usize, Win32Error> {
        match handle.0 {
            STD_OUTPUT_HANDLE_VALUE => self.guest_stdout.extend_from_slice(bytes),
            STD_ERROR_HANDLE_VALUE => self.guest_stderr.extend_from_slice(bytes),
            _ => return Err(Win32Error::InvalidHandle(handle.0)),
        }
        Ok(bytes.len())
    }

    fn seek_file(&mut self, handle: Handle, distance: i64, origin: u32) -> Result<u64, Win32Error> {
        self.process_io.seek(handle, distance, origin)
    }

    fn file_type(&self, handle: Handle) -> Option<u32> {
        if self.process_io.contains(handle) {
            Some(FILE_TYPE_DISK)
        } else if matches!(
            handle.0,
            STD_INPUT_HANDLE_VALUE | STD_OUTPUT_HANDLE_VALUE | STD_ERROR_HANDLE_VALUE
        ) {
            Some(FILE_TYPE_CHAR)
        } else {
            None
        }
    }

    fn environment_variable(&self, name: &str) -> Option<&str> {
        self.environment
            .get(&name.to_ascii_uppercase())
            .map(String::as_str)
    }

    fn environment_block_ansi(&self) -> GuestAddress {
        self.environment_block_ansi
    }

    fn environment_block_utf16(&self) -> GuestAddress {
        self.environment_block_utf16
    }

    fn current_directory(&self) -> &str {
        self.current_directory
    }

    fn set_current_directory(&mut self, path: &str) -> Result<(), Win32Error> {
        if path.is_empty() {
            return Err(Win32Error::InvalidArgument("empty current directory"));
        }
        *self.current_directory = path.replace('/', "\\");
        Ok(())
    }

    fn current_process_id(&self) -> u32 {
        GUEST_PROCESS_ID
    }

    fn current_thread_id(&self) -> u32 {
        GUEST_THREAD_ID
    }
}
