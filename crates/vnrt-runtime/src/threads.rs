use super::*;
use vnrt_x86::CpuState;

/// Default committed Guest stack for cooperative worker threads.
const WORKER_STACK_SIZE: u32 = 0x0004_0000;
/// Region reserved for worker stacks (below the initial thread stack).
const WORKER_STACK_REGION_BASE: u32 = 0x7f00_0000;
/// One TEB page per worker, growing downward from the initial TEB.
const WORKER_TEB_STRIDE: u32 = PAGE_SIZE_U32;
/// Region for per-thread dynamic TLS slot arrays.
const WORKER_TLS_REGION_BASE: u32 = 0x0ff0_0000;
const WORKER_TLS_STRIDE: u32 = PAGE_SIZE_U32;
const MAX_GUEST_THREADS: usize = 32;
/// CREATE_SUSPENDED
const CREATE_SUSPENDED: u32 = 0x0000_0004;
/// Host-visible return stub for a ThreadProc that falls off the end.
pub(super) const THREAD_EXIT_RETURN_ADDRESS: u32 = 0xffff_fff0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GuestThreadState {
    /// Eligible to run when selected by the cooperative scheduler.
    Ready,
    /// Currently executing on the single Host interpreter.
    Running,
    /// Blocked in a Guest wait that could not be satisfied immediately.
    Waiting,
    /// Finished; its handle remains signalled for Wait callers.
    Terminated,
}

#[derive(Debug, Clone)]
pub(super) struct PendingWait {
    pub(super) handles: Vec<Handle>,
    pub(super) wait_all: bool,
    pub(super) cleanup: u32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CompletedWait {
    pub(super) result: u32,
    pub(super) cleanup: u32,
}

#[derive(Debug)]
pub(super) struct GuestThread {
    pub(super) thread_id: u32,
    pub(super) handle: Handle,
    pub(super) state: GuestThreadState,
    pub(super) suspend_count: u32,
    pub(super) exit_code: u32,
    #[allow(dead_code)]
    pub(super) stack_base: u32,
    #[allow(dead_code)]
    pub(super) stack_size: u32,
    pub(super) teb_base: u32,
    pub(super) tls_base: u32,
    pub(super) cpu: CpuState,
    pub(super) last_error: u32,
    pub(super) pending_wait: Option<PendingWait>,
    /// Satisfied wait to complete when this thread is scheduled again.
    pub(super) completed_wait: Option<CompletedWait>,
}

#[derive(Debug)]
pub(super) struct ThreadManager {
    threads: BTreeMap<u32, GuestThread>,
    handle_to_id: BTreeMap<u32, u32>,
    current_id: u32,
    next_thread_id: u32,
    next_handle: u32,
    next_worker_index: u32,
}

impl ThreadManager {
    pub(super) fn new_main(cpu: &CpuState) -> Self {
        let main = GuestThread {
            thread_id: GUEST_THREAD_ID,
            handle: Handle(0),
            state: GuestThreadState::Running,
            suspend_count: 0,
            exit_code: 0,
            stack_base: GUEST_STACK_BASE,
            stack_size: GUEST_STACK_SIZE,
            teb_base: GUEST_TEB_BASE,
            tls_base: GUEST_TLS_BASE,
            cpu: cpu.clone(),
            last_error: 0,
            pending_wait: None,
            completed_wait: None,
        };
        let mut threads = BTreeMap::new();
        threads.insert(GUEST_THREAD_ID, main);
        Self {
            threads,
            handle_to_id: BTreeMap::new(),
            current_id: GUEST_THREAD_ID,
            next_thread_id: GUEST_THREAD_ID + 4,
            next_handle: 0x0004_0000,
            next_worker_index: 0,
        }
    }

    pub(super) fn current_id(&self) -> u32 {
        self.current_id
    }

    pub(super) fn current(&self) -> &GuestThread {
        self.threads
            .get(&self.current_id)
            .expect("current Guest thread must exist")
    }

    pub(super) fn current_mut(&mut self) -> &mut GuestThread {
        let id = self.current_id;
        self.threads
            .get_mut(&id)
            .expect("current Guest thread must exist")
    }

    pub(super) fn current_teb(&self) -> u32 {
        self.current().teb_base
    }

    pub(super) fn current_tls_base(&self) -> u32 {
        self.current().tls_base
    }

    pub(super) fn tls_bases(&self) -> Vec<u32> {
        self.threads.values().map(|thread| thread.tls_base).collect()
    }

    pub(super) fn thread_is_signaled(&self, handle: Handle) -> Option<bool> {
        let thread_id = *self.handle_to_id.get(&handle.0)?;
        let thread = self.threads.get(&thread_id)?;
        Some(thread.state == GuestThreadState::Terminated)
    }

    pub(super) fn close_handle(&mut self, handle: Handle) -> Result<(), Win32Error> {
        let thread_id = self
            .handle_to_id
            .remove(&handle.0)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        if let Some(thread) = self.threads.get_mut(&thread_id) {
            thread.handle = Handle(0);
        }
        Ok(())
    }

    pub(super) fn create_thread(
        &mut self,
        memory: &mut GuestMemory,
        start_address: GuestAddress,
        parameter: u32,
        creation_flags: u32,
        stack_size: u32,
    ) -> Result<(Handle, u32), Win32Error> {
        if start_address.0 == 0 {
            return Err(Win32Error::InvalidArgument("null thread start address"));
        }
        if self.threads.len() >= MAX_GUEST_THREADS {
            return Err(Win32Error::OutOfMemory);
        }
        let unsupported_flags = creation_flags & !CREATE_SUSPENDED;
        if unsupported_flags != 0 {
            return Err(Win32Error::Unsupported {
                feature: "CreateThread creation flags",
            });
        }

        let worker_index = self.next_worker_index;
        self.next_worker_index = self
            .next_worker_index
            .checked_add(1)
            .ok_or(Win32Error::OutOfMemory)?;
        let stack_size = if stack_size == 0 {
            WORKER_STACK_SIZE
        } else {
            stack_size
                .checked_add(PAGE_SIZE_U32 - 1)
                .map(|value| value & !(PAGE_SIZE_U32 - 1))
                .filter(|value| *value != 0)
                .ok_or(Win32Error::OutOfMemory)?
        };
        let stack_base = WORKER_STACK_REGION_BASE
            .checked_add(
                worker_index
                    .checked_mul(WORKER_STACK_SIZE)
                    .ok_or(Win32Error::OutOfMemory)?,
            )
            .ok_or(Win32Error::OutOfMemory)?;
        if stack_base
            .checked_add(stack_size)
            .filter(|end| *end <= GUEST_STACK_BASE)
            .is_none()
        {
            return Err(Win32Error::OutOfMemory);
        }
        let teb_base = GUEST_TEB_BASE
            .checked_sub(
                (worker_index + 1)
                    .checked_mul(WORKER_TEB_STRIDE)
                    .ok_or(Win32Error::OutOfMemory)?,
            )
            .ok_or(Win32Error::OutOfMemory)?;
        let tls_base = WORKER_TLS_REGION_BASE
            .checked_add(
                worker_index
                    .checked_mul(WORKER_TLS_STRIDE)
                    .ok_or(Win32Error::OutOfMemory)?,
            )
            .ok_or(Win32Error::OutOfMemory)?;

        memory
            .map_range(
                GuestAddress(stack_base),
                stack_size,
                Permissions::READ_WRITE,
            )
            .map_err(|_| Win32Error::OutOfMemory)?;
        memory
            .map_range(
                GuestAddress(teb_base),
                PAGE_SIZE_U32,
                Permissions::READ_WRITE,
            )
            .map_err(|_| Win32Error::OutOfMemory)?;
        memory
            .map_range(
                GuestAddress(tls_base),
                PAGE_SIZE_U32,
                Permissions::READ_WRITE,
            )
            .map_err(|_| Win32Error::OutOfMemory)?;

        let stack_top = stack_base
            .checked_add(stack_size)
            .ok_or(Win32Error::OutOfMemory)?;
        // ThreadProc(parameter): [esp]=return, [esp+4]=parameter
        let param_slot = stack_top.checked_sub(4).ok_or(Win32Error::OutOfMemory)?;
        let return_slot = stack_top.checked_sub(8).ok_or(Win32Error::OutOfMemory)?;
        memory
            .write_u32(GuestAddress(param_slot), parameter)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;
        memory
            .write_u32(GuestAddress(return_slot), THREAD_EXIT_RETURN_ADDRESS)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;

        let thread_id = self.next_thread_id;
        self.next_thread_id = self
            .next_thread_id
            .checked_add(4)
            .ok_or(Win32Error::HandleExhausted)?;
        let handle_value = self.next_handle;
        self.next_handle = self
            .next_handle
            .checked_add(4)
            .ok_or(Win32Error::HandleExhausted)?;
        let handle = Handle(handle_value);

        initialize_worker_teb(memory, teb_base, stack_base, stack_top, thread_id, tls_base)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;

        let mut cpu = CpuState::default();
        cpu.registers.eip = start_address.0;
        cpu.registers.esp = return_slot;
        cpu.fs_base = teb_base;

        let suspended = creation_flags & CREATE_SUSPENDED != 0;
        let thread = GuestThread {
            thread_id,
            handle,
            state: GuestThreadState::Ready,
            suspend_count: u32::from(suspended),
            exit_code: 0,
            stack_base,
            stack_size,
            teb_base,
            tls_base,
            cpu,
            last_error: 0,
            pending_wait: None,
            completed_wait: None,
        };
        self.threads.insert(thread_id, thread);
        self.handle_to_id.insert(handle_value, thread_id);
        Ok((handle, thread_id))
    }

    pub(super) fn resume(&mut self, handle: Handle) -> Result<u32, Win32Error> {
        if handle.0 == u32::MAX - 1 {
            let previous = self.current().suspend_count;
            if previous > 0 {
                self.current_mut().suspend_count -= 1;
            }
            return Ok(previous);
        }
        let thread_id = *self
            .handle_to_id
            .get(&handle.0)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        let thread = self
            .threads
            .get_mut(&thread_id)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        if thread.state == GuestThreadState::Terminated {
            return Err(Win32Error::InvalidHandle(handle.0));
        }
        let previous = thread.suspend_count;
        if previous > 0 {
            thread.suspend_count -= 1;
        }
        Ok(previous)
    }

    pub(super) fn park_current_wait(&mut self, wait: PendingWait) {
        let current = self.current_mut();
        current.state = GuestThreadState::Waiting;
        current.pending_wait = Some(wait);
        current.completed_wait = None;
    }

    /// Mark the current thread terminated. Returns true when it was the main thread.
    pub(super) fn exit_current(&mut self, exit_code: u32) -> bool {
        let current = self.current_mut();
        let is_main = current.thread_id == GUEST_THREAD_ID;
        current.exit_code = exit_code;
        current.state = GuestThreadState::Terminated;
        current.pending_wait = None;
        current.completed_wait = None;
        current.suspend_count = 0;
        is_main
    }

    /// Snapshot every thread currently blocked in a wait.
    pub(super) fn waiting_threads(&self) -> Vec<(u32, PendingWait)> {
        self.threads
            .iter()
            .filter_map(|(id, thread)| {
                (thread.state == GuestThreadState::Waiting)
                    .then(|| thread.pending_wait.clone().map(|wait| (*id, wait)))
                    .flatten()
            })
            .collect()
    }

    /// Complete a parked wait and mark the thread Ready.
    pub(super) fn complete_wait(
        &mut self,
        thread_id: u32,
        result: u32,
        cleanup: u32,
    ) -> Result<(), Win32Error> {
        let thread = self
            .threads
            .get_mut(&thread_id)
            .ok_or(Win32Error::InvalidArgument("unknown Guest thread"))?;
        if thread.state != GuestThreadState::Waiting {
            return Err(Win32Error::InvalidArgument(
                "complete_wait on non-waiting thread",
            ));
        }
        thread.pending_wait = None;
        thread.completed_wait = Some(CompletedWait { result, cleanup });
        thread.state = GuestThreadState::Ready;
        Ok(())
    }

    /// Pick another runnable thread, preferring lower thread ids for determinism.
    pub(super) fn pick_runnable_other(&self) -> Option<u32> {
        let mut candidates = self
            .threads
            .values()
            .filter(|thread| {
                thread.thread_id != self.current_id
                    && thread.state == GuestThreadState::Ready
                    && thread.suspend_count == 0
                    && thread.pending_wait.is_none()
            })
            .map(|thread| thread.thread_id)
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates.first().copied()
    }

    /// Pick any runnable thread including the current one.
    pub(super) fn pick_runnable_any(&self) -> Option<u32> {
        if let Some(id) = self.pick_runnable_other() {
            return Some(id);
        }
        let current = self.current();
        if matches!(
            current.state,
            GuestThreadState::Ready | GuestThreadState::Running
        ) && current.suspend_count == 0
            && current.pending_wait.is_none()
        {
            Some(current.thread_id)
        } else {
            None
        }
    }

    /// Save the live interpreter into the current thread, then activate `next_id`.
    pub(super) fn switch_to(
        &mut self,
        next_id: u32,
        cpu: &mut CpuState,
        last_error: &mut u32,
        memory: &mut GuestMemory,
    ) -> Result<(), Win32Error> {
        if next_id != self.current_id {
            if let Some(current) = self.threads.get_mut(&self.current_id) {
                if current.state == GuestThreadState::Running {
                    current.state = GuestThreadState::Ready;
                }
                current.cpu = cpu.clone();
                current.last_error = *last_error;
            }

            let next = self
                .threads
                .get_mut(&next_id)
                .ok_or(Win32Error::InvalidArgument("unknown Guest thread"))?;
            if next.suspend_count != 0 || next.pending_wait.is_some() {
                return Err(Win32Error::Unsupported {
                    feature: "scheduled a non-runnable Guest thread",
                });
            }
            *cpu = next.cpu.clone();
            *last_error = next.last_error;
            next.state = GuestThreadState::Running;
            self.current_id = next_id;
        } else {
            let current = self.current_mut();
            if current.state != GuestThreadState::Terminated {
                current.state = GuestThreadState::Running;
            }
        }

        if let Some(completed) = self.current_mut().completed_wait.take() {
            finish_wait_return(cpu, memory, completed)?;
        }

        memory
            .write_u32(
                GuestAddress(self.current().teb_base + TEB_LAST_ERROR_OFFSET),
                *last_error,
            )
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;
        Ok(())
    }
}

fn initialize_worker_teb(
    memory: &mut GuestMemory,
    teb_base: u32,
    stack_base: u32,
    stack_top: u32,
    thread_id: u32,
    tls_base: u32,
) -> Result<(), MemoryError> {
    memory.write_u32(GuestAddress(teb_base), u32::MAX)?;
    memory.write_u32(GuestAddress(teb_base + 0x04), stack_top)?;
    memory.write_u32(GuestAddress(teb_base + 0x08), stack_base)?;
    memory.write_u32(GuestAddress(teb_base + 0x18), teb_base)?;
    memory.write_u32(GuestAddress(teb_base + 0x20), GUEST_PROCESS_ID)?;
    memory.write_u32(GuestAddress(teb_base + 0x24), thread_id)?;
    memory.write_u32(GuestAddress(teb_base + 0x2c), tls_base)?;
    memory.write_u32(GuestAddress(teb_base + 0x30), GUEST_PEB_BASE)?;
    memory.write_u32(GuestAddress(teb_base + TEB_LAST_ERROR_OFFSET), 0)?;
    Ok(())
}

pub(super) fn finish_wait_return(
    cpu: &mut CpuState,
    memory: &mut GuestMemory,
    completed: CompletedWait,
) -> Result<(), Win32Error> {
    let stack = cpu.registers.esp;
    let return_address = memory
        .read_u32(GuestAddress(stack))
        .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;
    cpu.registers.esp = stack
        .checked_add(4)
        .and_then(|value| value.checked_add(completed.cleanup))
        .ok_or(Win32Error::InvalidArgument(
            "wait return stack pointer overflow",
        ))?;
    cpu.registers.eip = return_address;
    cpu.registers.eax = completed.result;
    Ok(())
}
