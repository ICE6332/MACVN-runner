use super::*;

pub(super) const EXCEPTION_HANDLER_RETURN_ADDRESS: u32 = 0xffff_fff4;

const EXCEPTION_BREAKPOINT: u32 = 0x8000_0003;
const EXCEPTION_ILLEGAL_INSTRUCTION: u32 = 0xc000_001d;
const EXCEPTION_ACCESS_VIOLATION: u32 = 0xc000_0005;
const EXCEPTION_RECORD_SIZE: u32 = 0x50;
const CONTEXT_SIZE: u32 = 0x2cc;
const CONTEXT_FLAGS: u32 = 0x0001_0007;
const MAX_EXCEPTION_CHAIN_DEPTH: usize = 64;

const CONTEXT_EDI: u32 = 156;
const CONTEXT_ESI: u32 = 160;
const CONTEXT_EBX: u32 = 164;
const CONTEXT_EDX: u32 = 168;
const CONTEXT_ECX: u32 = 172;
const CONTEXT_EAX: u32 = 176;
const CONTEXT_EBP: u32 = 180;
const CONTEXT_EIP: u32 = 184;
const CONTEXT_EFLAGS: u32 = 192;
const CONTEXT_ESP: u32 = 196;

pub(super) struct PendingException {
    code: u32,
    flags: u32,
    address: GuestAddress,
    record: GuestAddress,
    context: GuestAddress,
    registration: GuestAddress,
    visited_frames: usize,
}

impl Runtime {
    pub(super) fn dispatch_cpu_exception(
        &mut self,
        exception: CpuException,
    ) -> Result<(), RuntimeError> {
        if let Some(pending) = self.pending_exception.as_ref() {
            return Err(RuntimeError::UnsupportedNestedException {
                pending_code: pending.code,
                pending_address: pending.address.0,
                nested: format!("{exception:?}"),
                eip: self.cpu.state.registers.eip,
            });
        }
        let (code, address, information) = match exception {
            CpuException::Breakpoint { address, .. } => (EXCEPTION_BREAKPOINT, address, Vec::new()),
            CpuException::IllegalInstruction { address } => {
                (EXCEPTION_ILLEGAL_INSTRUCTION, address, Vec::new())
            }
            CpuException::AccessViolation {
                address,
                memory_address,
                access,
            } => (
                EXCEPTION_ACCESS_VIOLATION,
                address,
                vec![access, memory_address.0],
            ),
        };
        self.dispatch_guest_exception(code, 0, address, &information)
    }

    pub(super) fn dispatch_guest_exception(
        &mut self,
        code: u32,
        flags: u32,
        address: GuestAddress,
        information: &[u32],
    ) -> Result<(), RuntimeError> {
        if self.pending_exception.is_some() {
            return Err(RuntimeError::Unsupported(
                "nested Guest exception during SEH dispatch",
            ));
        }
        if information.len() > 15 {
            return Err(RuntimeError::Unsupported(
                "Guest exception parameter limit exceeded",
            ));
        }
        let fault_stack_words = (0..9_u32)
            .map_while(|index| {
                self.cpu
                    .state
                    .registers
                    .esp
                    .checked_add(index * 4)
                    .and_then(|word| self.memory.read_u32(GuestAddress(word)).ok())
            })
            .collect::<Vec<_>>();
        let fault_pointer_previews = fault_stack_words
            .iter()
            .filter_map(|address| preview_pointer(&self.memory, *address))
            .collect::<Vec<_>>();
        debug!(
            code,
            address = address.0,
            stack_words = ?fault_stack_words,
            stack_pointer_previews = ?fault_pointer_previews,
            "dispatching synchronous Guest exception"
        );
        let registration =
            GuestAddress(self.memory.read_u32(GuestAddress(self.cpu.state.fs_base))?);
        if registration.0 == u32::MAX {
            return Err(RuntimeError::UnhandledGuestException {
                code,
                address: address.0,
            });
        }

        let context = GuestAddress(
            self.cpu
                .state
                .registers
                .esp
                .checked_sub(CONTEXT_SIZE)
                .ok_or(RuntimeError::Unsupported("SEH context stack overflow"))?
                & !0xf,
        );
        let record = GuestAddress(
            context
                .0
                .checked_sub(EXCEPTION_RECORD_SIZE)
                .ok_or(RuntimeError::Unsupported("SEH record stack overflow"))?,
        );
        self.write_exception_record(record, code, flags, address, information)?;
        self.write_x86_context(context)?;
        self.pending_exception = Some(PendingException {
            code,
            flags,
            address,
            record,
            context,
            registration,
            visited_frames: 1,
        });
        self.enter_exception_handler()
    }

    pub(super) fn advance_exception_dispatch(&mut self) -> Result<(), RuntimeError> {
        let disposition = self.cpu.state.registers.eax;
        match disposition {
            0 => {
                if self
                    .pending_exception
                    .as_ref()
                    .is_some_and(|pending| pending.flags & 1 != 0)
                {
                    return Err(RuntimeError::Unsupported(
                        "continued a noncontinuable Guest exception",
                    ));
                }
                let context = self
                    .pending_exception
                    .as_ref()
                    .ok_or(RuntimeError::Unsupported(
                        "SEH return sentinel without a pending exception",
                    ))?
                    .context;
                self.restore_x86_context(context)?;
                self.pending_exception = None;
                Ok(())
            }
            1 => {
                let pending = self
                    .pending_exception
                    .as_mut()
                    .ok_or(RuntimeError::Unsupported(
                        "SEH return sentinel without a pending exception",
                    ))?;
                let next = GuestAddress(self.memory.read_u32(pending.registration)?);
                if next.0 == u32::MAX {
                    return Err(RuntimeError::UnhandledGuestException {
                        code: pending.code,
                        address: pending.address.0,
                    });
                }
                if next == pending.registration
                    || pending.visited_frames >= MAX_EXCEPTION_CHAIN_DEPTH
                {
                    return Err(RuntimeError::Unsupported("invalid Guest SEH chain"));
                }
                pending.registration = next;
                pending.visited_frames += 1;
                self.enter_exception_handler()
            }
            _ => Err(RuntimeError::Unsupported(
                "nested or collided Guest SEH disposition",
            )),
        }
    }

    fn enter_exception_handler(&mut self) -> Result<(), RuntimeError> {
        let pending = self
            .pending_exception
            .as_ref()
            .ok_or(RuntimeError::Unsupported("missing pending Guest exception"))?;
        let handler = GuestAddress(
            self.memory
                .read_u32(GuestAddress(pending.registration.0 + 4))?,
        );
        enter_stdcall_callback(
            &mut self.cpu,
            &mut self.memory,
            handler,
            &[
                pending.record.0,
                pending.registration.0,
                pending.context.0,
                0,
            ],
            pending.record.0,
        )?;
        let stack = self.cpu.state.registers.esp;
        self.memory
            .write_u32(GuestAddress(stack), EXCEPTION_HANDLER_RETURN_ADDRESS)?;
        debug!(
            handler = handler.0,
            record = pending.record.0,
            registration = pending.registration.0,
            context = pending.context.0,
            callback_frame = stack,
            "entered Guest exception handler"
        );
        Ok(())
    }

    fn write_exception_record(
        &mut self,
        record: GuestAddress,
        code: u32,
        flags: u32,
        address: GuestAddress,
        information: &[u32],
    ) -> Result<(), RuntimeError> {
        self.memory
            .write(record, &vec![0; EXCEPTION_RECORD_SIZE as usize])?;
        self.memory.write_u32(record, code)?;
        self.memory.write_u32(GuestAddress(record.0 + 4), flags)?;
        self.memory
            .write_u32(GuestAddress(record.0 + 12), address.0)?;
        self.memory.write_u32(
            GuestAddress(record.0 + 16),
            u32::try_from(information.len())
                .map_err(|_| RuntimeError::Unsupported("exception parameter count overflow"))?,
        )?;
        for (index, value) in information.iter().enumerate() {
            let offset = u32::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(4))
                .and_then(|offset| offset.checked_add(20))
                .ok_or(RuntimeError::Unsupported(
                    "exception parameter offset overflow",
                ))?;
            self.memory
                .write_u32(GuestAddress(record.0 + offset), *value)?;
        }
        Ok(())
    }

    fn write_x86_context(&mut self, context: GuestAddress) -> Result<(), RuntimeError> {
        self.memory
            .write(context, &vec![0; CONTEXT_SIZE as usize])?;
        self.memory.write_u32(context, CONTEXT_FLAGS)?;
        self.memory.write_u32(GuestAddress(context.0 + 144), 0x3b)?;
        self.memory.write_u32(GuestAddress(context.0 + 148), 0x23)?;
        self.memory.write_u32(GuestAddress(context.0 + 152), 0x23)?;
        let registers = self.cpu.state.registers;
        for (offset, value) in [
            (CONTEXT_EDI, registers.edi),
            (CONTEXT_ESI, registers.esi),
            (CONTEXT_EBX, registers.ebx),
            (CONTEXT_EDX, registers.edx),
            (CONTEXT_ECX, registers.ecx),
            (CONTEXT_EAX, registers.eax),
            (CONTEXT_EBP, registers.ebp),
            (CONTEXT_EIP, registers.eip),
            (188, 0x1b),
            (CONTEXT_EFLAGS, registers.eflags),
            (CONTEXT_ESP, registers.esp),
            (200, 0x23),
        ] {
            self.memory
                .write_u32(GuestAddress(context.0 + offset), value)?;
        }
        Ok(())
    }

    fn restore_x86_context(&mut self, context: GuestAddress) -> Result<(), RuntimeError> {
        let read = |memory: &GuestMemory, offset| memory.read_u32(GuestAddress(context.0 + offset));
        self.cpu.state.registers = Registers {
            edi: read(&self.memory, CONTEXT_EDI)?,
            esi: read(&self.memory, CONTEXT_ESI)?,
            ebx: read(&self.memory, CONTEXT_EBX)?,
            edx: read(&self.memory, CONTEXT_EDX)?,
            ecx: read(&self.memory, CONTEXT_ECX)?,
            eax: read(&self.memory, CONTEXT_EAX)?,
            ebp: read(&self.memory, CONTEXT_EBP)?,
            eip: read(&self.memory, CONTEXT_EIP)?,
            eflags: read(&self.memory, CONTEXT_EFLAGS)?,
            esp: read(&self.memory, CONTEXT_ESP)?,
        };
        Ok(())
    }
}

fn preview_pointer(memory: &GuestMemory, address: u32) -> Option<(u32, String)> {
    let mut bytes = [0_u8; 64];
    memory.read(GuestAddress(address), &mut bytes).ok()?;
    let ascii = bytes
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| {
            if byte.is_ascii_graphic() || *byte == b' ' {
                char::from(*byte)
            } else {
                '.'
            }
        })
        .collect::<String>();
    let utf16 = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    let utf16 = String::from_utf16(&utf16).unwrap_or_default();
    (!ascii.is_empty() || !utf16.is_empty())
        .then(|| (address, format!("ascii={ascii:?}, utf16={utf16:?}")))
}
