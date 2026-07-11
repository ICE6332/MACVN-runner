//! 32-bit x86 CPU state and an interpreter for the initial integer subset.
//!
//! This crate depends only on guest memory. In particular, it has no knowledge
//! of Win32 modules, handles, or APIs.

use std::{collections::HashMap, sync::Arc};

use iced_x86::{
    Code, Decoder, DecoderOptions, Instruction, MemorySize, Mnemonic, OpKind, Register,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use vnrt_memory::{GuestAddress, GuestMemory, MemoryError, PAGE_SIZE, PAGE_SIZE_U32};

/// Maximum length of one x86 instruction.
pub const MAX_INSTRUCTION_LEN: usize = 15;

/// Carry flag.
pub const FLAG_CF: u32 = 1 << 0;
/// Parity flag.
pub const FLAG_PF: u32 = 1 << 2;
/// Auxiliary carry flag.
pub const FLAG_AF: u32 = 1 << 4;
/// Zero flag.
pub const FLAG_ZF: u32 = 1 << 6;
/// Sign flag.
pub const FLAG_SF: u32 = 1 << 7;
/// String-instruction direction flag.
pub const FLAG_DF: u32 = 1 << 10;
/// Overflow flag.
pub const FLAG_OF: u32 = 1 << 11;

const ARITHMETIC_FLAGS: u32 = FLAG_CF | FLAG_PF | FLAG_AF | FLAG_ZF | FLAG_SF | FLAG_OF;
const LOGIC_FLAGS: u32 = FLAG_CF | FLAG_PF | FLAG_ZF | FLAG_SF | FLAG_OF;
const MAX_DECODE_CACHE_ENTRIES: usize = 1 << 20;
const MAX_BLOCK_CACHE_ENTRIES: usize = 1 << 18;
const MAX_BLOCK_INSTRUCTIONS: usize = 64;

/// Architectural integer registers used by the initial interpreter.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Registers {
    /// Accumulator.
    pub eax: u32,
    /// Base register.
    pub ebx: u32,
    /// Counter register.
    pub ecx: u32,
    /// Data register.
    pub edx: u32,
    /// Source index.
    pub esi: u32,
    /// Destination index.
    pub edi: u32,
    /// Stack pointer.
    pub esp: u32,
    /// Frame pointer.
    pub ebp: u32,
    /// Instruction pointer.
    pub eip: u32,
    /// Status and control flags.
    pub eflags: u32,
}

impl Default for Registers {
    fn default() -> Self {
        Self {
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
            esi: 0,
            edi: 0,
            esp: 0,
            ebp: 0,
            eip: 0,
            eflags: 0x2,
        }
    }
}

impl Registers {
    fn read(&self, register: Register) -> Option<u32> {
        match register {
            Register::EAX => Some(self.eax),
            Register::EBX => Some(self.ebx),
            Register::ECX => Some(self.ecx),
            Register::EDX => Some(self.edx),
            Register::ESI => Some(self.esi),
            Register::EDI => Some(self.edi),
            Register::ESP => Some(self.esp),
            Register::EBP => Some(self.ebp),
            Register::EIP => Some(self.eip),
            Register::AX => Some(self.eax & 0xffff),
            Register::BX => Some(self.ebx & 0xffff),
            Register::CX => Some(self.ecx & 0xffff),
            Register::DX => Some(self.edx & 0xffff),
            Register::SI => Some(self.esi & 0xffff),
            Register::DI => Some(self.edi & 0xffff),
            Register::SP => Some(self.esp & 0xffff),
            Register::BP => Some(self.ebp & 0xffff),
            Register::AL => Some(self.eax & 0xff),
            Register::BL => Some(self.ebx & 0xff),
            Register::CL => Some(self.ecx & 0xff),
            Register::DL => Some(self.edx & 0xff),
            Register::AH => Some((self.eax >> 8) & 0xff),
            Register::BH => Some((self.ebx >> 8) & 0xff),
            Register::CH => Some((self.ecx >> 8) & 0xff),
            Register::DH => Some((self.edx >> 8) & 0xff),
            _ => None,
        }
    }

    fn write(&mut self, register: Register, value: u32) -> bool {
        match register {
            Register::EAX => self.eax = value,
            Register::EBX => self.ebx = value,
            Register::ECX => self.ecx = value,
            Register::EDX => self.edx = value,
            Register::ESI => self.esi = value,
            Register::EDI => self.edi = value,
            Register::ESP => self.esp = value,
            Register::EBP => self.ebp = value,
            Register::EIP => self.eip = value,
            Register::AX => self.eax = (self.eax & 0xffff_0000) | (value & 0xffff),
            Register::BX => self.ebx = (self.ebx & 0xffff_0000) | (value & 0xffff),
            Register::CX => self.ecx = (self.ecx & 0xffff_0000) | (value & 0xffff),
            Register::DX => self.edx = (self.edx & 0xffff_0000) | (value & 0xffff),
            Register::SI => self.esi = (self.esi & 0xffff_0000) | (value & 0xffff),
            Register::DI => self.edi = (self.edi & 0xffff_0000) | (value & 0xffff),
            Register::SP => self.esp = (self.esp & 0xffff_0000) | (value & 0xffff),
            Register::BP => self.ebp = (self.ebp & 0xffff_0000) | (value & 0xffff),
            Register::AL => self.eax = (self.eax & 0xffff_ff00) | (value & 0xff),
            Register::BL => self.ebx = (self.ebx & 0xffff_ff00) | (value & 0xff),
            Register::CL => self.ecx = (self.ecx & 0xffff_ff00) | (value & 0xff),
            Register::DL => self.edx = (self.edx & 0xffff_ff00) | (value & 0xff),
            Register::AH => self.eax = (self.eax & 0xffff_00ff) | ((value & 0xff) << 8),
            Register::BH => self.ebx = (self.ebx & 0xffff_00ff) | ((value & 0xff) << 8),
            Register::CH => self.ecx = (self.ecx & 0xffff_00ff) | ((value & 0xff) << 8),
            Register::DH => self.edx = (self.edx & 0xffff_00ff) | ((value & 0xff) << 8),
            _ => return false,
        }
        true
    }

    fn flag(&self, flag: u32) -> bool {
        self.eflags & flag != 0
    }

    fn replace_flags(&mut self, mask: u32, values: u32) {
        self.eflags = (self.eflags & !mask) | (values & mask) | 0x2;
    }
}

/// Mutable CPU state, kept separate from decoder and dispatch policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CpuState {
    /// Integer registers.
    pub registers: Registers,
    /// Linear base added to memory operands carrying an `FS:` override.
    ///
    /// Win32 uses this segment for the current thread's TEB in 32-bit mode.
    pub fs_base: u32,
    /// x87 control word; reset state masks exceptions and uses extended precision.
    pub x87_control_word: u16,
    /// x87 status word, including exception and condition-code state.
    pub x87_status_word: u16,
    /// Physical MMX register payloads shared with the future x87 register file.
    pub mmx_registers: [u64; 8],
    /// Set after the guest terminates normally.
    pub halted: bool,
}

impl Default for CpuState {
    fn default() -> Self {
        Self {
            registers: Registers::default(),
            fs_base: 0,
            x87_control_word: 0x037f,
            x87_status_word: 0,
            mmx_registers: [0; 8],
            halted: false,
        }
    }
}

/// Address classification supplied by the outer runtime.
///
/// Returning `true` tells the interpreter that the current EIP is a host-owned
/// thunk. The CPU reports it without depending on the Win32 implementation.
pub trait ExternalTargetResolver {
    /// Whether execution at this address should be delegated to the host.
    fn is_external_target(&self, address: GuestAddress) -> bool;
}

/// Resolver used when no external targets have been installed.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoExternalTargets;

impl ExternalTargetResolver for NoExternalTargets {
    fn is_external_target(&self, _address: GuestAddress) -> bool {
        false
    }
}

/// Result of executing or classifying one instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// One guest instruction was executed.
    Continue {
        /// Decoded instruction, useful to debugger clients.
        instruction: Instruction,
    },
    /// Control reached a host-owned thunk.
    ExternalCall {
        /// Address used by the runtime to locate a registered API.
        address: GuestAddress,
    },
    /// Guest execution raised a synchronous processor exception.
    Exception {
        /// Exception reported to the outer runtime.
        exception: CpuException,
    },
    /// Execution was already halted.
    Halted,
}

/// Result of executing consecutive instructions until a Runtime boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchOutcome {
    /// The complete requested instruction budget was consumed.
    BudgetExhausted {
        /// Guest steps consumed by the batch.
        steps: u64,
    },
    /// Control reached a host-owned thunk.
    ExternalCall {
        /// Host thunk address.
        address: GuestAddress,
        /// Guest steps consumed including the boundary transition.
        steps: u64,
    },
    /// Guest execution raised a synchronous processor exception.
    Exception {
        /// Exception reported to the outer runtime.
        exception: CpuException,
        /// Guest steps consumed including the faulting instruction.
        steps: u64,
    },
    /// Execution reached the halted state.
    Halted {
        /// Guest steps consumed including the halt observation.
        steps: u64,
    },
}

/// Synchronous processor exceptions that require operating-system dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuException {
    /// Software breakpoint raised by `INT3`.
    Breakpoint {
        /// Address of the one-byte breakpoint instruction.
        address: GuestAddress,
        /// Architectural EIP reported in the exception context.
        resume_address: GuestAddress,
    },
}

/// CPU decode or execution errors.
#[derive(Debug, Error)]
pub enum CpuError {
    /// Guest memory could not provide instruction, operand, or stack bytes.
    #[error(transparent)]
    Memory(#[from] MemoryError),
    /// `iced-x86` reported an invalid instruction stream.
    #[error("invalid x86 instruction at {address:#010x}")]
    InvalidInstruction {
        /// Guest instruction address.
        address: u32,
    },
    /// The instruction is valid but is not in the current interpreter subset.
    #[error("unsupported x86 instruction {instruction} at {address:#010x}")]
    UnsupportedInstruction {
        /// Instruction enum name.
        instruction: String,
        /// Guest instruction address.
        address: u32,
    },
    /// An operand form or width is outside the current 32-bit subset.
    #[error("unsupported x86 operand at {address:#010x}: {detail}")]
    UnsupportedOperand {
        /// Guest instruction address.
        address: u32,
        /// Unsupported form.
        detail: String,
    },
    /// A future CPU facility has not been implemented.
    #[error("unsupported CPU operation: {0}")]
    Unsupported(&'static str),
    /// Hardware-style divide fault caused by a zero divisor or oversized quotient.
    #[error("x86 divide error at {address:#010x}: {detail}")]
    DivideError {
        /// Faulting instruction address.
        address: u32,
        /// Specific arithmetic condition.
        detail: &'static str,
    },
}

#[derive(Debug, Clone, Copy)]
enum OperandTarget {
    Register(Register),
    Memory(GuestAddress),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperandWidth {
    Byte,
    Word,
    Dword,
}

impl OperandWidth {
    const fn bits(self) -> u32 {
        match self {
            Self::Byte => 8,
            Self::Word => 16,
            Self::Dword => 32,
        }
    }

    const fn mask(self) -> u32 {
        match self {
            Self::Byte => 0xff,
            Self::Word => 0xffff,
            Self::Dword => u32::MAX,
        }
    }

    const fn sign_bit(self) -> u32 {
        1 << (self.bits() - 1)
    }

    const fn truncate(self, value: u32) -> u32 {
        value & self.mask()
    }

    const fn sign_extend(self, value: u32) -> u32 {
        let value = self.truncate(value);
        if value & self.sign_bit() != 0 {
            value | !self.mask()
        } else {
            value
        }
    }
}

/// Single-step 32-bit x86 interpreter.
#[derive(Debug, Clone, Default)]
pub struct Interpreter {
    /// Architectural state exposed to the runtime and debugger.
    pub state: CpuState,
    instruction_cache: HashMap<u32, CachedInstruction>,
    block_cache: HashMap<u32, Arc<CachedBlock>>,
    block_hotness: HashMap<u32, u8>,
}

#[derive(Debug, Clone)]
struct CachedInstruction {
    instruction: Instruction,
    first_page_generation: u64,
    second_page_generation: Option<u64>,
}

#[derive(Debug)]
struct CachedBlock {
    instructions: Vec<Instruction>,
    page_generations: Vec<(GuestAddress, u64)>,
}

impl Interpreter {
    /// Create an interpreter beginning at `entry_point`.
    #[must_use]
    pub fn new(entry_point: GuestAddress) -> Self {
        let mut state = CpuState::default();
        state.registers.eip = entry_point.0;
        Self {
            state,
            instruction_cache: HashMap::new(),
            block_cache: HashMap::new(),
            block_hotness: HashMap::new(),
        }
    }

    /// Execute one instruction or report an external call boundary.
    pub fn step(
        &mut self,
        memory: &mut GuestMemory,
        resolver: &impl ExternalTargetResolver,
    ) -> Result<StepOutcome, CpuError> {
        if self.state.halted {
            return Ok(StepOutcome::Halted);
        }

        let eip = GuestAddress(self.state.registers.eip);
        if resolver.is_external_target(eip) {
            return Ok(StepOutcome::ExternalCall { address: eip });
        }

        let instruction = self.decode_cached(memory, eip)?;
        if instruction.mnemonic() == Mnemonic::Int3 {
            let resume_address = GuestAddress(instruction.next_ip32());
            self.state.registers.eip = resume_address.0;
            return Ok(StepOutcome::Exception {
                exception: CpuException::Breakpoint {
                    address: eip,
                    resume_address,
                },
            });
        }
        self.execute(memory, &instruction)?;
        Ok(StepOutcome::Continue { instruction })
    }

    /// Execute until a Host/exception/halt boundary or `max_steps` is consumed.
    pub fn run_batch(
        &mut self,
        memory: &mut GuestMemory,
        resolver: &impl ExternalTargetResolver,
        max_steps: u64,
    ) -> Result<BatchOutcome, CpuError> {
        let mut steps = 0;
        while steps < max_steps {
            if self.state.halted {
                return Ok(BatchOutcome::Halted { steps: steps + 1 });
            }
            let eip = GuestAddress(self.state.registers.eip);
            if resolver.is_external_target(eip) {
                return Ok(BatchOutcome::ExternalCall {
                    address: eip,
                    steps: steps + 1,
                });
            }
            let Some(block) = self.decode_block(memory, eip)? else {
                match self.step(memory, resolver)? {
                    StepOutcome::Continue { .. } => {
                        steps += 1;
                        continue;
                    }
                    StepOutcome::ExternalCall { address } => {
                        return Ok(BatchOutcome::ExternalCall {
                            address,
                            steps: steps + 1,
                        });
                    }
                    StepOutcome::Exception { exception } => {
                        return Ok(BatchOutcome::Exception {
                            exception,
                            steps: steps + 1,
                        });
                    }
                    StepOutcome::Halted => {
                        return Ok(BatchOutcome::Halted { steps: steps + 1 });
                    }
                }
            };
            for instruction in &block.instructions {
                if steps >= max_steps || instruction.ip32() != self.state.registers.eip {
                    break;
                }
                if instruction.mnemonic() == Mnemonic::Int3 {
                    let address = GuestAddress(instruction.ip32());
                    let resume_address = GuestAddress(instruction.next_ip32());
                    self.state.registers.eip = resume_address.0;
                    return Ok(BatchOutcome::Exception {
                        exception: CpuException::Breakpoint {
                            address,
                            resume_address,
                        },
                        steps: steps + 1,
                    });
                }
                self.execute(memory, instruction)?;
                steps += 1;
                if !block_is_valid(memory, &block)? {
                    break;
                }
            }
        }
        Ok(BatchOutcome::BudgetExhausted { steps })
    }

    fn decode_block(
        &mut self,
        memory: &GuestMemory,
        start: GuestAddress,
    ) -> Result<Option<Arc<CachedBlock>>, CpuError> {
        if let Some(block) = self.block_cache.get(&start.0).cloned()
            && block_is_valid(memory, &block)?
        {
            return Ok(Some(block));
        }
        if self.block_cache.remove(&start.0).is_some() {
            self.block_hotness.remove(&start.0);
            return Ok(None);
        }
        let hotness = self.block_hotness.entry(start.0).or_default();
        *hotness = hotness.saturating_add(1);
        if *hotness < 2 {
            return Ok(None);
        }

        let mut instructions = Vec::with_capacity(MAX_BLOCK_INSTRUCTIONS);
        let mut page_generations = Vec::new();
        let mut address = start;
        for _ in 0..MAX_BLOCK_INSTRUCTIONS {
            record_code_pages(memory, address, 1, &mut page_generations)?;
            let instruction = self.decode_cached(memory, address)?;
            record_code_pages(memory, address, instruction.len(), &mut page_generations)?;
            let terminal = is_block_terminal(&instruction);
            address = GuestAddress(instruction.next_ip32());
            instructions.push(instruction);
            if terminal {
                break;
            }
        }
        let block = Arc::new(CachedBlock {
            instructions,
            page_generations,
        });
        if self.block_cache.len() >= MAX_BLOCK_CACHE_ENTRIES {
            self.block_cache.clear();
            self.block_hotness.clear();
        }
        self.block_cache.insert(start.0, Arc::clone(&block));
        Ok(Some(block))
    }

    fn decode_cached(
        &mut self,
        memory: &GuestMemory,
        eip: GuestAddress,
    ) -> Result<Instruction, CpuError> {
        let first_page_generation = memory.executable_page_generation(eip)?;
        if let Some(cached) = self.instruction_cache.get(&eip.0)
            && cached.first_page_generation == first_page_generation
            && second_page_generation(memory, eip, cached.instruction.len())?
                == cached.second_page_generation
        {
            return Ok(cached.instruction);
        }

        let instruction = decode(memory, eip)?;
        let second_page_generation = second_page_generation(memory, eip, instruction.len())?;
        if self.instruction_cache.len() >= MAX_DECODE_CACHE_ENTRIES {
            self.instruction_cache.clear();
        }
        self.instruction_cache.insert(
            eip.0,
            CachedInstruction {
                instruction,
                first_page_generation,
                second_page_generation,
            },
        );
        Ok(instruction)
    }

    fn execute(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
    ) -> Result<(), CpuError> {
        let next_ip = instruction.next_ip32();
        match instruction.mnemonic() {
            Mnemonic::Nop => self.state.registers.eip = next_ip,
            Mnemonic::Wait => self.state.registers.eip = next_ip,
            Mnemonic::Fnstcw => {
                let address = self.effective_address(instruction, true)?;
                memory.write_u16(address, self.state.x87_control_word)?;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Fldcw => {
                let address = self.effective_address(instruction, true)?;
                self.state.x87_control_word = memory.read_u16(address)?;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Fnclex => {
                self.state.x87_status_word &= !0x80ff;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Movd => self.execute_mmx_movd(memory, instruction)?,
            Mnemonic::Movq => self.execute_mmx_movq(memory, instruction)?,
            Mnemonic::Punpckldq => self.execute_mmx_punpckldq(memory, instruction)?,
            Mnemonic::Emms => self.state.registers.eip = next_ip,
            Mnemonic::Cld => {
                self.state.registers.eflags &= !FLAG_DF;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Std => {
                self.state.registers.eflags |= FLAG_DF;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Hlt => {
                self.state.registers.eip = next_ip;
                self.state.halted = true;
            }
            Mnemonic::Mov => {
                let value = self.read_operand(memory, instruction, 1)?;
                let target = self.operand_target(instruction, 0)?;
                self.write_target(memory, instruction, target, value)?;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Xchg => {
                let left = self.read_operand(memory, instruction, 0)?;
                let right = self.read_operand(memory, instruction, 1)?;
                let left_target = self.operand_target(instruction, 0)?;
                let right_target = self.operand_target(instruction, 1)?;
                self.write_target(memory, instruction, left_target, right)?;
                self.write_target(memory, instruction, right_target, left)?;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Movzx | Mnemonic::Movsx => {
                let source_width = self.operand_width(instruction, 1)?;
                let mut value = self.read_operand(memory, instruction, 1)?;
                if instruction.mnemonic() == Mnemonic::Movsx {
                    value = source_width.sign_extend(value);
                }
                let target = self.operand_target(instruction, 0)?;
                self.write_target(memory, instruction, target, value)?;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Cwde => {
                self.state.registers.eax = OperandWidth::Word.sign_extend(self.state.registers.eax);
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Cdq => {
                self.state.registers.edx = if self.state.registers.eax & 0x8000_0000 != 0 {
                    u32::MAX
                } else {
                    0
                };
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Cwd => {
                let high = if self.state.registers.eax & 0x8000 != 0 {
                    0xffff
                } else {
                    0
                };
                self.state.registers.edx = (self.state.registers.edx & 0xffff_0000) | high;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Lea => {
                let target = self.operand_target(instruction, 0)?;
                // LEA calculates only the effective offset; segment bases are
                // applied by actual memory accesses, not address arithmetic.
                let address = self.effective_address(instruction, false)?;
                self.write_target(memory, instruction, target, address.0)?;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Add => self.execute_binary(memory, instruction, BinaryOperation::Add)?,
            Mnemonic::Adc => self.execute_binary(memory, instruction, BinaryOperation::Adc)?,
            Mnemonic::Sub => self.execute_binary(memory, instruction, BinaryOperation::Sub)?,
            Mnemonic::Sbb => self.execute_binary(memory, instruction, BinaryOperation::Sbb)?,
            Mnemonic::Cmp => self.execute_binary(memory, instruction, BinaryOperation::Compare)?,
            Mnemonic::Test => self.execute_binary(memory, instruction, BinaryOperation::Test)?,
            Mnemonic::And => self.execute_binary(memory, instruction, BinaryOperation::And)?,
            Mnemonic::Or => self.execute_binary(memory, instruction, BinaryOperation::Or)?,
            Mnemonic::Xor => self.execute_binary(memory, instruction, BinaryOperation::Xor)?,
            Mnemonic::Shl | Mnemonic::Sal => self.execute_shift_left(memory, instruction)?,
            Mnemonic::Shr => self.execute_shift_right(memory, instruction, false)?,
            Mnemonic::Sar => self.execute_shift_right(memory, instruction, true)?,
            Mnemonic::Rol => self.execute_rotate(memory, instruction, true)?,
            Mnemonic::Ror => self.execute_rotate(memory, instruction, false)?,
            Mnemonic::Neg => self.execute_negate(memory, instruction)?,
            Mnemonic::Not => self.execute_not(memory, instruction)?,
            Mnemonic::Imul => self.execute_imul(memory, instruction)?,
            Mnemonic::Mul => self.execute_mul(memory, instruction)?,
            Mnemonic::Div => self.execute_divide(memory, instruction, false)?,
            Mnemonic::Idiv => self.execute_divide(memory, instruction, true)?,
            mnemonic if is_string_mnemonic(mnemonic) && has_string_memory_operand(instruction) => {
                self.execute_string(memory, instruction)?;
            }
            Mnemonic::Inc => self.execute_increment(memory, instruction, false)?,
            Mnemonic::Dec => self.execute_increment(memory, instruction, true)?,
            Mnemonic::Push => {
                let value = self.read_operand(memory, instruction, 0)?;
                self.push(memory, value)?;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Pushfd => {
                self.push(memory, self.state.registers.eflags)?;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Pop => {
                let value = self.pop(memory)?;
                let target = self.operand_target(instruction, 0)?;
                self.write_target(memory, instruction, target, value)?;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Popfd => {
                const WRITABLE_FLAGS: u32 = ARITHMETIC_FLAGS | FLAG_DF;
                let value = self.pop(memory)?;
                self.state.registers.eflags = (self.state.registers.eflags & !WRITABLE_FLAGS)
                    | (value & WRITABLE_FLAGS)
                    | 0x2;
                self.state.registers.eip = next_ip;
            }
            Mnemonic::Leave => self.execute_leave(memory, instruction)?,
            Mnemonic::Call => {
                let target = self.read_branch_target(memory, instruction, 0)?;
                self.push(memory, next_ip)?;
                self.state.registers.eip = target;
            }
            Mnemonic::Ret => {
                let target = self.pop(memory)?;
                let cleanup = if instruction.op_count() == 0 {
                    0
                } else {
                    self.read_operand(memory, instruction, 0)?
                };
                self.state.registers.esp = self.state.registers.esp.wrapping_add(cleanup);
                self.state.registers.eip = target;
            }
            Mnemonic::Jmp => {
                self.state.registers.eip = self.read_branch_target(memory, instruction, 0)?;
            }
            mnemonic if is_conditional_jump(mnemonic) => {
                self.state.registers.eip = if self.condition(mnemonic) {
                    self.read_branch_target(memory, instruction, 0)?
                } else {
                    next_ip
                };
            }
            mnemonic if is_setcc(mnemonic) => {
                let value = u32::from(self.condition(mnemonic));
                let target = self.operand_target(instruction, 0)?;
                self.write_target(memory, instruction, target, value)?;
                self.state.registers.eip = next_ip;
            }
            mnemonic if is_cmovcc(mnemonic) => {
                if self.condition(mnemonic) {
                    let value = self.read_operand(memory, instruction, 1)?;
                    let target = self.operand_target(instruction, 0)?;
                    self.write_target(memory, instruction, target, value)?;
                }
                self.state.registers.eip = next_ip;
            }
            _ => {
                return Err(CpuError::UnsupportedInstruction {
                    instruction: format!("{:?}", instruction.code()),
                    address: instruction.ip32(),
                });
            }
        }
        Ok(())
    }

    fn execute_binary(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
        operation: BinaryOperation,
    ) -> Result<(), CpuError> {
        let left = self.read_operand(memory, instruction, 0)?;
        let right = self.read_operand(memory, instruction, 1)?;
        let width = self.operand_width(instruction, 0)?;
        let left = width.truncate(left);
        let right = width.truncate(right);
        let (result, flags, writes_result) = match operation {
            BinaryOperation::Add | BinaryOperation::Adc => {
                let carry = u32::from(
                    matches!(operation, BinaryOperation::Adc) && self.state.registers.flag(FLAG_CF),
                );
                let result = width.truncate(left.wrapping_add(right).wrapping_add(carry));
                (
                    result,
                    add_with_carry_flags(left, right, carry, result, width),
                    true,
                )
            }
            BinaryOperation::Sub | BinaryOperation::Sbb | BinaryOperation::Compare => {
                let borrow = u32::from(
                    matches!(operation, BinaryOperation::Sbb) && self.state.registers.flag(FLAG_CF),
                );
                let result = width.truncate(left.wrapping_sub(right).wrapping_sub(borrow));
                (
                    result,
                    sub_with_borrow_flags(left, right, borrow, result, width),
                    matches!(operation, BinaryOperation::Sub | BinaryOperation::Sbb),
                )
            }
            BinaryOperation::And | BinaryOperation::Test => {
                let result = width.truncate(left & right);
                (
                    result,
                    logic_flags(result, width),
                    matches!(operation, BinaryOperation::And),
                )
            }
            BinaryOperation::Or => {
                let result = width.truncate(left | right);
                (result, logic_flags(result, width), true)
            }
            BinaryOperation::Xor => {
                let result = width.truncate(left ^ right);
                (result, logic_flags(result, width), true)
            }
        };
        if writes_result {
            let target = self.operand_target(instruction, 0)?;
            self.write_target(memory, instruction, target, result)?;
        }
        let mask = if matches!(
            operation,
            BinaryOperation::And
                | BinaryOperation::Or
                | BinaryOperation::Xor
                | BinaryOperation::Test
        ) {
            LOGIC_FLAGS
        } else {
            ARITHMETIC_FLAGS
        };
        self.state.registers.replace_flags(mask, flags);
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn execute_increment(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
        decrement: bool,
    ) -> Result<(), CpuError> {
        let old = self.read_operand(memory, instruction, 0)?;
        let width = self.operand_width(instruction, 0)?;
        let old = width.truncate(old);
        let result = if decrement {
            width.truncate(old.wrapping_sub(1))
        } else {
            width.truncate(old.wrapping_add(1))
        };
        let flags = if decrement {
            sub_flags(old, 1, result, width)
        } else {
            add_flags(old, 1, result, width)
        };
        let old_carry = self.state.registers.eflags & FLAG_CF;
        self.state
            .registers
            .replace_flags(ARITHMETIC_FLAGS, (flags & !FLAG_CF) | old_carry);
        let target = self.operand_target(instruction, 0)?;
        self.write_target(memory, instruction, target, result)?;
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn execute_shift_left(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
    ) -> Result<(), CpuError> {
        let width = self.operand_width(instruction, 0)?;
        let value = width.truncate(self.read_operand(memory, instruction, 0)?);
        let count = self.read_operand(memory, instruction, 1)? & 0x1f;
        if count != 0 {
            let result = width.truncate(value.wrapping_shl(count));
            let carry = if count <= width.bits() {
                (value >> (width.bits() - count)) & 1 != 0
            } else {
                false
            };
            let mut flags = common_flags(result, width);
            if carry {
                flags |= FLAG_CF;
            }
            if count == 1 && ((result & width.sign_bit() != 0) != carry) {
                flags |= FLAG_OF;
            }
            self.state.registers.replace_flags(LOGIC_FLAGS, flags);
            let target = self.operand_target(instruction, 0)?;
            self.write_target(memory, instruction, target, result)?;
        }
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn execute_shift_right(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
        arithmetic: bool,
    ) -> Result<(), CpuError> {
        let width = self.operand_width(instruction, 0)?;
        let value = width.truncate(self.read_operand(memory, instruction, 0)?);
        let count = self.read_operand(memory, instruction, 1)? & 0x1f;
        if count != 0 {
            let result = if arithmetic {
                width.truncate(((width.sign_extend(value) as i32) >> count) as u32)
            } else {
                width.truncate(value >> count)
            };
            let carry = count <= width.bits() && ((value >> (count - 1)) & 1 != 0);
            let mut flags = common_flags(result, width);
            if carry {
                flags |= FLAG_CF;
            }
            if !arithmetic && count == 1 && value & width.sign_bit() != 0 {
                flags |= FLAG_OF;
            }
            self.state.registers.replace_flags(LOGIC_FLAGS, flags);
            let target = self.operand_target(instruction, 0)?;
            self.write_target(memory, instruction, target, result)?;
        }
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn execute_rotate(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
        left: bool,
    ) -> Result<(), CpuError> {
        let width = self.operand_width(instruction, 0)?;
        let value = width.truncate(self.read_operand(memory, instruction, 0)?);
        let count = (self.read_operand(memory, instruction, 1)? & 0x1f) % width.bits();
        if count != 0 {
            let result = if left {
                width.truncate((value << count) | (value >> (width.bits() - count)))
            } else {
                width.truncate((value >> count) | (value << (width.bits() - count)))
            };
            let carry = if left {
                result & 1 != 0
            } else {
                result & width.sign_bit() != 0
            };
            let mut flags = u32::from(carry) * FLAG_CF;
            let mut mask = FLAG_CF;
            if count == 1 {
                let overflow = if left {
                    (result & width.sign_bit() != 0) != carry
                } else {
                    let most_significant = result & width.sign_bit() != 0;
                    let next_significant = result & (width.sign_bit() >> 1) != 0;
                    most_significant != next_significant
                };
                if overflow {
                    flags |= FLAG_OF;
                }
                mask |= FLAG_OF;
            }
            self.state.registers.replace_flags(mask, flags);
            let target = self.operand_target(instruction, 0)?;
            self.write_target(memory, instruction, target, result)?;
        }
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn execute_negate(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
    ) -> Result<(), CpuError> {
        let width = self.operand_width(instruction, 0)?;
        let value = width.truncate(self.read_operand(memory, instruction, 0)?);
        let result = width.truncate(0_u32.wrapping_sub(value));
        let flags = sub_flags(0, value, result, width);
        let target = self.operand_target(instruction, 0)?;
        self.write_target(memory, instruction, target, result)?;
        self.state.registers.replace_flags(ARITHMETIC_FLAGS, flags);
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn execute_not(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
    ) -> Result<(), CpuError> {
        let width = self.operand_width(instruction, 0)?;
        let value = width.truncate(self.read_operand(memory, instruction, 0)?);
        let target = self.operand_target(instruction, 0)?;
        self.write_target(memory, instruction, target, width.truncate(!value))?;
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn execute_imul(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
    ) -> Result<(), CpuError> {
        let width = self.operand_width(instruction, 0)?;
        if instruction.op_count() == 1 {
            let left = signed_value(self.state.registers.eax, width);
            let right = signed_value(self.read_operand(memory, instruction, 0)?, width);
            let result = left * right;
            self.write_implicit_product(result as u64, width);
            self.set_multiply_overflow(result < signed_min(width) || result > signed_max(width));
        } else {
            let (left_index, right_index) = if instruction.op_count() == 2 {
                (0, 1)
            } else {
                (1, 2)
            };
            let left = signed_value(self.read_operand(memory, instruction, left_index)?, width);
            let right = signed_value(self.read_operand(memory, instruction, right_index)?, width);
            let result = left * right;
            let target = self.operand_target(instruction, 0)?;
            self.write_target(memory, instruction, target, result as u32)?;
            self.set_multiply_overflow(result < signed_min(width) || result > signed_max(width));
        }
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn execute_mul(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
    ) -> Result<(), CpuError> {
        let width = self.operand_width(instruction, 0)?;
        let left = u64::from(width.truncate(self.state.registers.eax));
        let right = u64::from(width.truncate(self.read_operand(memory, instruction, 0)?));
        let result = left * right;
        self.write_implicit_product(result, width);
        self.set_multiply_overflow(result > u64::from(width.mask()));
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn write_implicit_product(&mut self, result: u64, width: OperandWidth) {
        match width {
            OperandWidth::Byte => {
                self.state.registers.eax =
                    (self.state.registers.eax & 0xffff_0000) | (result as u32 & 0xffff);
            }
            OperandWidth::Word => {
                self.state.registers.eax =
                    (self.state.registers.eax & 0xffff_0000) | (result as u32 & 0xffff);
                self.state.registers.edx =
                    (self.state.registers.edx & 0xffff_0000) | ((result as u32 >> 16) & 0xffff);
            }
            OperandWidth::Dword => {
                self.state.registers.eax = result as u32;
                self.state.registers.edx = (result >> 32) as u32;
            }
        }
    }

    fn set_multiply_overflow(&mut self, overflow: bool) {
        let flags = if overflow { FLAG_CF | FLAG_OF } else { 0 };
        self.state.registers.replace_flags(FLAG_CF | FLAG_OF, flags);
    }

    fn execute_divide(
        &mut self,
        memory: &GuestMemory,
        instruction: &Instruction,
        signed: bool,
    ) -> Result<(), CpuError> {
        let width = self.operand_width(instruction, 0)?;
        let divisor = width.truncate(self.read_operand(memory, instruction, 0)?);
        if divisor == 0 {
            return Err(divide_error(instruction, "division by zero"));
        }
        if signed {
            self.execute_signed_divide(instruction, width, divisor)?;
        } else {
            self.execute_unsigned_divide(instruction, width, divisor)?;
        }
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn execute_unsigned_divide(
        &mut self,
        instruction: &Instruction,
        width: OperandWidth,
        divisor: u32,
    ) -> Result<(), CpuError> {
        let dividend = match width {
            OperandWidth::Byte => u64::from(self.state.registers.eax & 0xffff),
            OperandWidth::Word => u64::from(
                ((self.state.registers.edx & 0xffff) << 16) | (self.state.registers.eax & 0xffff),
            ),
            OperandWidth::Dword => {
                (u64::from(self.state.registers.edx) << 32) | u64::from(self.state.registers.eax)
            }
        };
        let quotient = dividend / u64::from(divisor);
        let remainder = dividend % u64::from(divisor);
        if quotient > u64::from(width.mask()) {
            return Err(divide_error(instruction, "unsigned quotient overflow"));
        }
        self.write_division_result(quotient as u32, remainder as u32, width);
        Ok(())
    }

    fn execute_signed_divide(
        &mut self,
        instruction: &Instruction,
        width: OperandWidth,
        divisor: u32,
    ) -> Result<(), CpuError> {
        let dividend = match width {
            OperandWidth::Byte => i64::from((self.state.registers.eax as u16) as i16),
            OperandWidth::Word => i64::from(
                (((self.state.registers.edx & 0xffff) << 16) | (self.state.registers.eax & 0xffff))
                    as i32,
            ),
            OperandWidth::Dword => {
                ((u64::from(self.state.registers.edx) << 32) | u64::from(self.state.registers.eax))
                    as i64
            }
        };
        let divisor = signed_value(divisor, width);
        let quotient = dividend
            .checked_div(divisor)
            .ok_or_else(|| divide_error(instruction, "signed quotient overflow"))?;
        let remainder = dividend
            .checked_rem(divisor)
            .ok_or_else(|| divide_error(instruction, "signed quotient overflow"))?;
        if quotient < signed_min(width) || quotient > signed_max(width) {
            return Err(divide_error(instruction, "signed quotient overflow"));
        }
        self.write_division_result(quotient as u32, remainder as u32, width);
        Ok(())
    }

    fn write_division_result(&mut self, quotient: u32, remainder: u32, width: OperandWidth) {
        match width {
            OperandWidth::Byte => {
                let low = (quotient & 0xff) | ((remainder & 0xff) << 8);
                self.state.registers.eax = (self.state.registers.eax & 0xffff_0000) | low;
            }
            OperandWidth::Word => {
                self.state.registers.eax =
                    (self.state.registers.eax & 0xffff_0000) | (quotient & 0xffff);
                self.state.registers.edx =
                    (self.state.registers.edx & 0xffff_0000) | (remainder & 0xffff);
            }
            OperandWidth::Dword => {
                self.state.registers.eax = quotient;
                self.state.registers.edx = remainder;
            }
        }
    }

    fn execute_string(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
    ) -> Result<(), CpuError> {
        validate_32_bit_string_addressing(instruction)?;
        let repeated = instruction.has_rep_prefix() || instruction.has_repne_prefix();
        if repeated && self.state.registers.ecx == 0 {
            self.state.registers.eip = instruction.next_ip32();
            return Ok(());
        }

        let mnemonic = instruction.mnemonic();
        let width = string_width(mnemonic)
            .ok_or_else(|| unsupported_operand(instruction, "unsupported string width"))?;
        let source = self.string_source_address(instruction)?;
        let destination = GuestAddress(self.state.registers.edi);
        match mnemonic {
            Mnemonic::Movsb | Mnemonic::Movsw | Mnemonic::Movsd => {
                let value = read_memory_width(memory, source, width)?;
                write_memory_width(memory, destination, value, width)?;
                self.advance_string_source(width);
                self.advance_string_destination(width);
            }
            Mnemonic::Stosb | Mnemonic::Stosw | Mnemonic::Stosd => {
                let value = width.truncate(self.state.registers.eax);
                write_memory_width(memory, destination, value, width)?;
                self.advance_string_destination(width);
            }
            Mnemonic::Lodsb | Mnemonic::Lodsw | Mnemonic::Lodsd => {
                let value = read_memory_width(memory, source, width)?;
                self.state
                    .registers
                    .write(accumulator_register(width), value);
                self.advance_string_source(width);
            }
            Mnemonic::Cmpsb | Mnemonic::Cmpsw | Mnemonic::Cmpsd => {
                let left = read_memory_width(memory, source, width)?;
                let right = read_memory_width(memory, destination, width)?;
                let result = width.truncate(left.wrapping_sub(right));
                self.state
                    .registers
                    .replace_flags(ARITHMETIC_FLAGS, sub_flags(left, right, result, width));
                self.advance_string_source(width);
                self.advance_string_destination(width);
            }
            Mnemonic::Scasb | Mnemonic::Scasw | Mnemonic::Scasd => {
                let left = width.truncate(self.state.registers.eax);
                let right = read_memory_width(memory, destination, width)?;
                let result = width.truncate(left.wrapping_sub(right));
                self.state
                    .registers
                    .replace_flags(ARITHMETIC_FLAGS, sub_flags(left, right, result, width));
                self.advance_string_destination(width);
            }
            _ => {
                return Err(unsupported_operand(
                    instruction,
                    "unsupported string instruction",
                ));
            }
        }

        if repeated {
            self.state.registers.ecx = self.state.registers.ecx.wrapping_sub(1);
            let compare = matches!(
                mnemonic,
                Mnemonic::Cmpsb
                    | Mnemonic::Cmpsw
                    | Mnemonic::Cmpsd
                    | Mnemonic::Scasb
                    | Mnemonic::Scasw
                    | Mnemonic::Scasd
            );
            let condition_allows_repeat = if !compare {
                true
            } else if instruction.has_repne_prefix() {
                !self.state.registers.flag(FLAG_ZF)
            } else {
                self.state.registers.flag(FLAG_ZF)
            };
            self.state.registers.eip = if self.state.registers.ecx != 0 && condition_allows_repeat {
                instruction.ip32()
            } else {
                instruction.next_ip32()
            };
        } else {
            self.state.registers.eip = instruction.next_ip32();
        }
        Ok(())
    }

    fn string_source_address(&self, instruction: &Instruction) -> Result<GuestAddress, CpuError> {
        let base = match instruction.segment_prefix() {
            Register::None => 0,
            Register::FS => self.state.fs_base,
            _ => {
                return Err(unsupported_operand(
                    instruction,
                    "unsupported string source segment override",
                ));
            }
        };
        Ok(GuestAddress(base.wrapping_add(self.state.registers.esi)))
    }

    fn advance_string_source(&mut self, width: OperandWidth) {
        self.state.registers.esi = adjust_string_index(
            self.state.registers.esi,
            width,
            self.state.registers.flag(FLAG_DF),
        );
    }

    fn advance_string_destination(&mut self, width: OperandWidth) {
        self.state.registers.edi = adjust_string_index(
            self.state.registers.edi,
            width,
            self.state.registers.flag(FLAG_DF),
        );
    }

    fn read_operand(
        &self,
        memory: &GuestMemory,
        instruction: &Instruction,
        index: u32,
    ) -> Result<u32, CpuError> {
        match instruction.op_kind(index) {
            OpKind::Register => self
                .state
                .registers
                .read(instruction.op_register(index))
                .ok_or_else(|| unsupported_operand(instruction, "non-32-bit register")),
            OpKind::Immediate8 => Ok(u32::from(instruction.immediate8())),
            OpKind::Immediate16 => Ok(u32::from(instruction.immediate16())),
            OpKind::Immediate32 => Ok(instruction.immediate32()),
            OpKind::Immediate8to16 => Ok(instruction.immediate8to16() as u32),
            OpKind::Immediate8to32 => Ok(instruction.immediate8to32() as u32),
            OpKind::NearBranch16 => Ok(u32::from(instruction.near_branch16())),
            OpKind::NearBranch32 => Ok(instruction.near_branch32()),
            OpKind::Memory => {
                let address = self.effective_address(instruction, true)?;
                match self.operand_width(instruction, index)? {
                    OperandWidth::Byte => {
                        memory.read_u8(address).map(u32::from).map_err(Into::into)
                    }
                    OperandWidth::Word => {
                        memory.read_u16(address).map(u32::from).map_err(Into::into)
                    }
                    OperandWidth::Dword => memory.read_u32(address).map_err(Into::into),
                }
            }
            kind => Err(unsupported_operand(
                instruction,
                &format!("unsupported operand kind {kind:?}"),
            )),
        }
    }

    fn read_branch_target(
        &self,
        memory: &GuestMemory,
        instruction: &Instruction,
        index: u32,
    ) -> Result<u32, CpuError> {
        match instruction.op_kind(index) {
            OpKind::NearBranch16 => Ok(u32::from(instruction.near_branch16())),
            OpKind::NearBranch32 => Ok(instruction.near_branch32()),
            _ => self.read_operand(memory, instruction, index),
        }
    }

    fn operand_target(
        &self,
        instruction: &Instruction,
        index: u32,
    ) -> Result<OperandTarget, CpuError> {
        match instruction.op_kind(index) {
            OpKind::Register => {
                let register = instruction.op_register(index);
                if self.state.registers.read(register).is_none() {
                    return Err(unsupported_operand(
                        instruction,
                        "non-32-bit register target",
                    ));
                }
                Ok(OperandTarget::Register(register))
            }
            OpKind::Memory => {
                self.operand_width(instruction, index)?;
                Ok(OperandTarget::Memory(
                    self.effective_address(instruction, true)?,
                ))
            }
            kind => Err(unsupported_operand(
                instruction,
                &format!("operand kind {kind:?} is not writable"),
            )),
        }
    }

    fn write_target(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
        target: OperandTarget,
        value: u32,
    ) -> Result<(), CpuError> {
        match target {
            OperandTarget::Register(register) => {
                if !self.state.registers.write(register, value) {
                    return Err(unsupported_operand(
                        instruction,
                        "non-32-bit register target",
                    ));
                }
                Ok(())
            }
            OperandTarget::Memory(address) => match self.operand_width(instruction, 0)? {
                OperandWidth::Byte => memory.write_u8(address, value as u8).map_err(Into::into),
                OperandWidth::Word => memory.write_u16(address, value as u16).map_err(Into::into),
                OperandWidth::Dword => memory.write_u32(address, value).map_err(Into::into),
            },
        }
    }

    fn effective_address(
        &self,
        instruction: &Instruction,
        apply_segment: bool,
    ) -> Result<GuestAddress, CpuError> {
        let mut address = instruction.memory_displacement32();
        let base = instruction.memory_base();
        if base != Register::None {
            address = address.wrapping_add(
                self.state
                    .registers
                    .read(base)
                    .ok_or_else(|| unsupported_operand(instruction, "unsupported memory base"))?,
            );
        }
        let index = instruction.memory_index();
        if index != Register::None {
            let index_value = self
                .state
                .registers
                .read(index)
                .ok_or_else(|| unsupported_operand(instruction, "unsupported memory index"))?;
            address =
                address.wrapping_add(index_value.wrapping_mul(instruction.memory_index_scale()));
        }
        if apply_segment {
            match instruction.segment_prefix() {
                Register::None => {}
                Register::FS => address = address.wrapping_add(self.state.fs_base),
                _ => {
                    return Err(unsupported_operand(
                        instruction,
                        "unsupported explicit segment override",
                    ));
                }
            }
        }
        Ok(GuestAddress(address))
    }

    fn operand_width(
        &self,
        instruction: &Instruction,
        index: u32,
    ) -> Result<OperandWidth, CpuError> {
        match instruction.op_kind(index) {
            OpKind::Register => register_width(instruction.op_register(index))
                .ok_or_else(|| unsupported_operand(instruction, "unsupported register width")),
            OpKind::Memory => match instruction.memory_size() {
                MemorySize::UInt8 | MemorySize::Int8 => Ok(OperandWidth::Byte),
                MemorySize::UInt16 | MemorySize::Int16 => Ok(OperandWidth::Word),
                MemorySize::UInt32 | MemorySize::Int32 | MemorySize::DwordOffset => {
                    Ok(OperandWidth::Dword)
                }
                size => Err(unsupported_operand(
                    instruction,
                    &format!("unsupported memory width {size:?}"),
                )),
            },
            kind => Err(unsupported_operand(
                instruction,
                &format!("operand kind {kind:?} has no intrinsic width"),
            )),
        }
    }

    fn push(&mut self, memory: &mut GuestMemory, value: u32) -> Result<(), CpuError> {
        let stack = self.state.registers.esp.wrapping_sub(4);
        memory.write_u32(GuestAddress(stack), value)?;
        self.state.registers.esp = stack;
        Ok(())
    }

    fn pop(&mut self, memory: &GuestMemory) -> Result<u32, CpuError> {
        let stack = self.state.registers.esp;
        let value = memory.read_u32(GuestAddress(stack))?;
        self.state.registers.esp = stack.wrapping_add(4);
        Ok(value)
    }

    fn execute_leave(
        &mut self,
        memory: &GuestMemory,
        instruction: &Instruction,
    ) -> Result<(), CpuError> {
        self.state.registers.esp = self.state.registers.ebp;
        match instruction.code() {
            Code::Leaved => self.state.registers.ebp = self.pop(memory)?,
            Code::Leavew => {
                let value = u32::from(memory.read_u16(GuestAddress(self.state.registers.esp))?);
                self.state.registers.esp = self.state.registers.esp.wrapping_add(2);
                self.state.registers.ebp = (self.state.registers.ebp & 0xffff_0000) | value;
            }
            _ => {
                return Err(unsupported_operand(
                    instruction,
                    "unsupported LEAVE operand size",
                ));
            }
        }
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn execute_mmx_movd(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
    ) -> Result<(), CpuError> {
        match instruction.code() {
            Code::Movd_mm_rm32 => {
                let destination = mmx_index(instruction.op_register(0))
                    .ok_or_else(|| unsupported_operand(instruction, "MOVD MMX destination"))?;
                self.state.mmx_registers[destination] =
                    u64::from(self.read_operand(memory, instruction, 1)?);
            }
            Code::Movd_rm32_mm => {
                let source = mmx_index(instruction.op_register(1))
                    .ok_or_else(|| unsupported_operand(instruction, "MOVD MMX source"))?;
                let target = self.operand_target(instruction, 0)?;
                self.write_target(
                    memory,
                    instruction,
                    target,
                    self.state.mmx_registers[source] as u32,
                )?;
            }
            _ => return Err(unsupported_operand(instruction, "non-MMX MOVD form")),
        }
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn execute_mmx_movq(
        &mut self,
        memory: &mut GuestMemory,
        instruction: &Instruction,
    ) -> Result<(), CpuError> {
        match instruction.code() {
            Code::Movq_mm_mmm64 => {
                let destination = mmx_index(instruction.op_register(0))
                    .ok_or_else(|| unsupported_operand(instruction, "MOVQ MMX destination"))?;
                self.state.mmx_registers[destination] =
                    self.read_mmx_or_memory(memory, instruction, 1)?;
            }
            Code::Movq_mmm64_mm => {
                let source = mmx_index(instruction.op_register(1))
                    .ok_or_else(|| unsupported_operand(instruction, "MOVQ MMX source"))?;
                let value = self.state.mmx_registers[source].to_le_bytes();
                let address = self.effective_address(instruction, true)?;
                memory.write(address, &value)?;
            }
            _ => return Err(unsupported_operand(instruction, "non-MMX MOVQ form")),
        }
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn execute_mmx_punpckldq(
        &mut self,
        memory: &GuestMemory,
        instruction: &Instruction,
    ) -> Result<(), CpuError> {
        if instruction.code() != Code::Punpckldq_mm_mmm32 {
            return Err(unsupported_operand(instruction, "non-MMX PUNPCKLDQ form"));
        }
        let destination = mmx_index(instruction.op_register(0))
            .ok_or_else(|| unsupported_operand(instruction, "PUNPCKLDQ destination"))?;
        let source = self.read_mmx_or_memory(memory, instruction, 1)?;
        let low = self.state.mmx_registers[destination] as u32;
        self.state.mmx_registers[destination] = u64::from(low) | (source & 0xffff_ffff) << 32;
        self.state.registers.eip = instruction.next_ip32();
        Ok(())
    }

    fn read_mmx_or_memory(
        &self,
        memory: &GuestMemory,
        instruction: &Instruction,
        index: u32,
    ) -> Result<u64, CpuError> {
        match instruction.op_kind(index) {
            OpKind::Register => mmx_index(instruction.op_register(index))
                .map(|index| self.state.mmx_registers[index])
                .ok_or_else(|| unsupported_operand(instruction, "MMX source register")),
            OpKind::Memory => {
                let address = self.effective_address(instruction, true)?;
                let mut bytes = [0; 8];
                memory.read(address, &mut bytes)?;
                Ok(u64::from_le_bytes(bytes))
            }
            _ => Err(unsupported_operand(instruction, "MMX source operand")),
        }
    }

    fn condition(&self, mnemonic: Mnemonic) -> bool {
        let flags = &self.state.registers;
        match mnemonic {
            Mnemonic::Jo | Mnemonic::Seto | Mnemonic::Cmovo => flags.flag(FLAG_OF),
            Mnemonic::Jno | Mnemonic::Setno | Mnemonic::Cmovno => !flags.flag(FLAG_OF),
            Mnemonic::Jb | Mnemonic::Setb | Mnemonic::Cmovb => flags.flag(FLAG_CF),
            Mnemonic::Jae | Mnemonic::Setae | Mnemonic::Cmovae => !flags.flag(FLAG_CF),
            Mnemonic::Je | Mnemonic::Sete | Mnemonic::Cmove => flags.flag(FLAG_ZF),
            Mnemonic::Jne | Mnemonic::Setne | Mnemonic::Cmovne => !flags.flag(FLAG_ZF),
            Mnemonic::Jbe | Mnemonic::Setbe | Mnemonic::Cmovbe => {
                flags.flag(FLAG_CF) || flags.flag(FLAG_ZF)
            }
            Mnemonic::Ja | Mnemonic::Seta | Mnemonic::Cmova => {
                !flags.flag(FLAG_CF) && !flags.flag(FLAG_ZF)
            }
            Mnemonic::Js | Mnemonic::Sets | Mnemonic::Cmovs => flags.flag(FLAG_SF),
            Mnemonic::Jns | Mnemonic::Setns | Mnemonic::Cmovns => !flags.flag(FLAG_SF),
            Mnemonic::Jp | Mnemonic::Setp | Mnemonic::Cmovp => flags.flag(FLAG_PF),
            Mnemonic::Jnp | Mnemonic::Setnp | Mnemonic::Cmovnp => !flags.flag(FLAG_PF),
            Mnemonic::Jl | Mnemonic::Setl | Mnemonic::Cmovl => {
                flags.flag(FLAG_SF) != flags.flag(FLAG_OF)
            }
            Mnemonic::Jge | Mnemonic::Setge | Mnemonic::Cmovge => {
                flags.flag(FLAG_SF) == flags.flag(FLAG_OF)
            }
            Mnemonic::Jle | Mnemonic::Setle | Mnemonic::Cmovle => {
                flags.flag(FLAG_ZF) || flags.flag(FLAG_SF) != flags.flag(FLAG_OF)
            }
            Mnemonic::Jg | Mnemonic::Setg | Mnemonic::Cmovg => {
                !flags.flag(FLAG_ZF) && flags.flag(FLAG_SF) == flags.flag(FLAG_OF)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum BinaryOperation {
    Add,
    Adc,
    Sub,
    Sbb,
    Compare,
    Test,
    And,
    Or,
    Xor,
}

fn decode(memory: &GuestMemory, eip: GuestAddress) -> Result<Instruction, CpuError> {
    let mut bytes = [0; MAX_INSTRUCTION_LEN];
    memory.fetch(eip, &mut bytes)?;
    let mut decoder = Decoder::with_ip(32, &bytes, u64::from(eip.0), DecoderOptions::NONE);
    let instruction = decoder.decode();
    if instruction.is_invalid() {
        return Err(CpuError::InvalidInstruction { address: eip.0 });
    }
    Ok(instruction)
}

fn second_page_generation(
    memory: &GuestMemory,
    eip: GuestAddress,
    instruction_len: usize,
) -> Result<Option<u64>, CpuError> {
    if eip.page_offset() + instruction_len <= PAGE_SIZE {
        return Ok(None);
    }
    let next_page = eip
        .page_base()
        .0
        .checked_add(PAGE_SIZE_U32)
        .map(GuestAddress)
        .ok_or(MemoryError::AddressOverflow)?;
    memory
        .executable_page_generation(next_page)
        .map(Some)
        .map_err(Into::into)
}

fn block_is_valid(memory: &GuestMemory, block: &CachedBlock) -> Result<bool, CpuError> {
    for (page, generation) in &block.page_generations {
        if memory.executable_page_generation(*page)? != *generation {
            return Ok(false);
        }
    }
    Ok(true)
}

fn record_code_pages(
    memory: &GuestMemory,
    address: GuestAddress,
    instruction_len: usize,
    generations: &mut Vec<(GuestAddress, u64)>,
) -> Result<(), CpuError> {
    let first = address.page_base();
    if !generations.iter().any(|(page, _)| *page == first) {
        generations.push((first, memory.executable_page_generation(first)?));
    }
    if address.page_offset() + instruction_len > PAGE_SIZE {
        let second = first
            .0
            .checked_add(PAGE_SIZE_U32)
            .map(GuestAddress)
            .ok_or(MemoryError::AddressOverflow)?;
        if !generations.iter().any(|(page, _)| *page == second) {
            generations.push((second, memory.executable_page_generation(second)?));
        }
    }
    Ok(())
}

fn is_block_terminal(instruction: &Instruction) -> bool {
    matches!(
        instruction.mnemonic(),
        Mnemonic::Call | Mnemonic::Ret | Mnemonic::Jmp | Mnemonic::Hlt | Mnemonic::Int3
    ) || is_conditional_jump(instruction.mnemonic())
        || is_string_mnemonic(instruction.mnemonic())
}

fn unsupported_operand(instruction: &Instruction, detail: &str) -> CpuError {
    CpuError::UnsupportedOperand {
        address: instruction.ip32(),
        detail: detail.to_owned(),
    }
}

fn divide_error(instruction: &Instruction, detail: &'static str) -> CpuError {
    CpuError::DivideError {
        address: instruction.ip32(),
        detail,
    }
}

fn signed_value(value: u32, width: OperandWidth) -> i64 {
    i64::from(width.sign_extend(value) as i32)
}

fn signed_min(width: OperandWidth) -> i64 {
    -(1_i64 << (width.bits() - 1))
}

fn signed_max(width: OperandWidth) -> i64 {
    (1_i64 << (width.bits() - 1)) - 1
}

fn is_conditional_jump(mnemonic: Mnemonic) -> bool {
    matches!(
        mnemonic,
        Mnemonic::Jo
            | Mnemonic::Jno
            | Mnemonic::Jb
            | Mnemonic::Jae
            | Mnemonic::Je
            | Mnemonic::Jne
            | Mnemonic::Jbe
            | Mnemonic::Ja
            | Mnemonic::Js
            | Mnemonic::Jns
            | Mnemonic::Jp
            | Mnemonic::Jnp
            | Mnemonic::Jl
            | Mnemonic::Jge
            | Mnemonic::Jle
            | Mnemonic::Jg
    )
}

fn is_setcc(mnemonic: Mnemonic) -> bool {
    matches!(
        mnemonic,
        Mnemonic::Seto
            | Mnemonic::Setno
            | Mnemonic::Setb
            | Mnemonic::Setae
            | Mnemonic::Sete
            | Mnemonic::Setne
            | Mnemonic::Setbe
            | Mnemonic::Seta
            | Mnemonic::Sets
            | Mnemonic::Setns
            | Mnemonic::Setp
            | Mnemonic::Setnp
            | Mnemonic::Setl
            | Mnemonic::Setge
            | Mnemonic::Setle
            | Mnemonic::Setg
    )
}

fn is_cmovcc(mnemonic: Mnemonic) -> bool {
    matches!(
        mnemonic,
        Mnemonic::Cmovo
            | Mnemonic::Cmovno
            | Mnemonic::Cmovb
            | Mnemonic::Cmovae
            | Mnemonic::Cmove
            | Mnemonic::Cmovne
            | Mnemonic::Cmovbe
            | Mnemonic::Cmova
            | Mnemonic::Cmovs
            | Mnemonic::Cmovns
            | Mnemonic::Cmovp
            | Mnemonic::Cmovnp
            | Mnemonic::Cmovl
            | Mnemonic::Cmovge
            | Mnemonic::Cmovle
            | Mnemonic::Cmovg
    )
}

fn is_string_mnemonic(mnemonic: Mnemonic) -> bool {
    matches!(
        mnemonic,
        Mnemonic::Movsb
            | Mnemonic::Movsw
            | Mnemonic::Movsd
            | Mnemonic::Stosb
            | Mnemonic::Stosw
            | Mnemonic::Stosd
            | Mnemonic::Lodsb
            | Mnemonic::Lodsw
            | Mnemonic::Lodsd
            | Mnemonic::Cmpsb
            | Mnemonic::Cmpsw
            | Mnemonic::Cmpsd
            | Mnemonic::Scasb
            | Mnemonic::Scasw
            | Mnemonic::Scasd
    )
}

fn has_string_memory_operand(instruction: &Instruction) -> bool {
    (0..instruction.op_count()).any(|index| {
        matches!(
            instruction.op_kind(index),
            OpKind::MemorySegSI
                | OpKind::MemorySegESI
                | OpKind::MemorySegRSI
                | OpKind::MemoryESDI
                | OpKind::MemoryESEDI
                | OpKind::MemoryESRDI
        )
    })
}

fn string_width(mnemonic: Mnemonic) -> Option<OperandWidth> {
    match mnemonic {
        Mnemonic::Movsb | Mnemonic::Stosb | Mnemonic::Lodsb | Mnemonic::Cmpsb | Mnemonic::Scasb => {
            Some(OperandWidth::Byte)
        }
        Mnemonic::Movsw | Mnemonic::Stosw | Mnemonic::Lodsw | Mnemonic::Cmpsw | Mnemonic::Scasw => {
            Some(OperandWidth::Word)
        }
        Mnemonic::Movsd | Mnemonic::Stosd | Mnemonic::Lodsd | Mnemonic::Cmpsd | Mnemonic::Scasd => {
            Some(OperandWidth::Dword)
        }
        _ => None,
    }
}

fn validate_32_bit_string_addressing(instruction: &Instruction) -> Result<(), CpuError> {
    for index in 0..instruction.op_count() {
        if matches!(
            instruction.op_kind(index),
            OpKind::MemorySegSI | OpKind::MemoryESDI | OpKind::MemorySegRSI | OpKind::MemoryESRDI
        ) {
            return Err(unsupported_operand(
                instruction,
                "only 32-bit string addressing is supported",
            ));
        }
    }
    Ok(())
}

fn accumulator_register(width: OperandWidth) -> Register {
    match width {
        OperandWidth::Byte => Register::AL,
        OperandWidth::Word => Register::AX,
        OperandWidth::Dword => Register::EAX,
    }
}

fn adjust_string_index(index: u32, width: OperandWidth, decrement: bool) -> u32 {
    if decrement {
        index.wrapping_sub(width.bits() / 8)
    } else {
        index.wrapping_add(width.bits() / 8)
    }
}

fn read_memory_width(
    memory: &GuestMemory,
    address: GuestAddress,
    width: OperandWidth,
) -> Result<u32, CpuError> {
    match width {
        OperandWidth::Byte => memory.read_u8(address).map(u32::from).map_err(Into::into),
        OperandWidth::Word => memory.read_u16(address).map(u32::from).map_err(Into::into),
        OperandWidth::Dword => memory.read_u32(address).map_err(Into::into),
    }
}

fn write_memory_width(
    memory: &mut GuestMemory,
    address: GuestAddress,
    value: u32,
    width: OperandWidth,
) -> Result<(), CpuError> {
    match width {
        OperandWidth::Byte => memory.write_u8(address, value as u8).map_err(Into::into),
        OperandWidth::Word => memory.write_u16(address, value as u16).map_err(Into::into),
        OperandWidth::Dword => memory.write_u32(address, value).map_err(Into::into),
    }
}

fn register_width(register: Register) -> Option<OperandWidth> {
    match register {
        Register::AL
        | Register::AH
        | Register::BL
        | Register::BH
        | Register::CL
        | Register::CH
        | Register::DL
        | Register::DH => Some(OperandWidth::Byte),
        Register::AX
        | Register::BX
        | Register::CX
        | Register::DX
        | Register::SI
        | Register::DI
        | Register::SP
        | Register::BP => Some(OperandWidth::Word),
        Register::EAX
        | Register::EBX
        | Register::ECX
        | Register::EDX
        | Register::ESI
        | Register::EDI
        | Register::ESP
        | Register::EBP
        | Register::EIP => Some(OperandWidth::Dword),
        _ => None,
    }
}

fn mmx_index(register: Register) -> Option<usize> {
    match register {
        Register::MM0 => Some(0),
        Register::MM1 => Some(1),
        Register::MM2 => Some(2),
        Register::MM3 => Some(3),
        Register::MM4 => Some(4),
        Register::MM5 => Some(5),
        Register::MM6 => Some(6),
        Register::MM7 => Some(7),
        _ => None,
    }
}

fn add_flags(left: u32, right: u32, result: u32, width: OperandWidth) -> u32 {
    add_with_carry_flags(left, right, 0, result, width)
}

fn add_with_carry_flags(
    left: u32,
    right: u32,
    carry: u32,
    result: u32,
    width: OperandWidth,
) -> u32 {
    let mut flags = common_flags(result, width);
    if u64::from(left) + u64::from(right) + u64::from(carry) > u64::from(width.mask()) {
        flags |= FLAG_CF;
    }
    if (left & 0xf) + (right & 0xf) + carry > 0xf {
        flags |= FLAG_AF;
    }
    let signed_result = signed_value(left, width) + signed_value(right, width) + i64::from(carry);
    if signed_result < signed_min(width) || signed_result > signed_max(width) {
        flags |= FLAG_OF;
    }
    flags
}

fn sub_flags(left: u32, right: u32, result: u32, width: OperandWidth) -> u32 {
    sub_with_borrow_flags(left, right, 0, result, width)
}

fn sub_with_borrow_flags(
    left: u32,
    right: u32,
    borrow: u32,
    result: u32,
    width: OperandWidth,
) -> u32 {
    let mut flags = common_flags(result, width);
    if u64::from(left) < u64::from(right) + u64::from(borrow) {
        flags |= FLAG_CF;
    }
    if (left & 0xf) < (right & 0xf) + borrow {
        flags |= FLAG_AF;
    }
    let signed_result = signed_value(left, width) - signed_value(right, width) - i64::from(borrow);
    if signed_result < signed_min(width) || signed_result > signed_max(width) {
        flags |= FLAG_OF;
    }
    flags
}

fn logic_flags(result: u32, width: OperandWidth) -> u32 {
    common_flags(result, width)
}

fn common_flags(result: u32, width: OperandWidth) -> u32 {
    let result = width.truncate(result);
    let mut flags = 0;
    if result == 0 {
        flags |= FLAG_ZF;
    }
    if result & width.sign_bit() != 0 {
        flags |= FLAG_SF;
    }
    if (result as u8).count_ones().is_multiple_of(2) {
        flags |= FLAG_PF;
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnrt_memory::Permissions;

    #[test]
    fn executes_integer_stack_and_branch_sequence() {
        // mov eax,1; add eax,2; cmp eax,3; jne fail; push eax; pop ebx; nop
        let code = [
            0xb8, 1, 0, 0, 0, 0x83, 0xc0, 2, 0x83, 0xf8, 3, 0x75, 3, 0x50, 0x5b, 0x90,
        ];
        let (mut cpu, mut memory) = machine(&code);
        for _ in 0..7 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }
        assert_eq!(cpu.state.registers.eax, 3);
        assert_eq!(cpu.state.registers.ebx, 3);
        assert!(cpu.state.registers.flag(FLAG_ZF));
        assert_eq!(cpu.state.registers.esp, 0x3000);
        assert_eq!(cpu.state.registers.eip, 0x1010);
    }

    #[test]
    fn calls_and_returns_through_the_guest_stack() {
        // call +1; nop; mov eax,42; ret
        let code = [0xe8, 1, 0, 0, 0, 0x90, 0xb8, 42, 0, 0, 0, 0xc3];
        let (mut cpu, mut memory) = machine(&code);
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(cpu.state.registers.eip, 0x1006);
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(cpu.state.registers.eax, 42);
        assert_eq!(cpu.state.registers.eip, 0x1005);
        assert_eq!(cpu.state.registers.esp, 0x3000);
    }

    #[test]
    fn pushfd_popfd_preserve_the_386_feature_persona() {
        // pushfd; pop eax; push 00200246h (ID bit set); popfd
        let code = [0x9c, 0x58, 0x68, 0x46, 0x02, 0x20, 0x00, 0x9d];
        let (mut cpu, mut memory) = machine(&code);
        cpu.state.registers.eflags = 0x246;
        for _ in 0..4 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }
        assert_eq!(cpu.state.registers.eax, 0x246);
        assert_eq!(cpu.state.registers.eflags, 0x246);
        assert_eq!(cpu.state.registers.esp, 0x3000);
    }

    #[test]
    fn leave_restores_the_callers_frame() {
        // push ebp; mov ebp,esp; sub esp,16; leave
        let code = [0x55, 0x89, 0xe5, 0x83, 0xec, 0x10, 0xc9];
        let (mut cpu, mut memory) = machine(&code);
        cpu.state.registers.ebp = 0x1234_5678;
        for _ in 0..4 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }
        assert_eq!(cpu.state.registers.ebp, 0x1234_5678);
        assert_eq!(cpu.state.registers.esp, 0x3000);
        assert_eq!(cpu.state.registers.eip, 0x1007);
    }

    #[test]
    fn wait_without_pending_x87_exception_advances() {
        let (mut cpu, mut memory) = machine(&[0x9b]);
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(cpu.state.registers.eip, 0x1001);
    }

    #[test]
    fn x87_control_word_round_trips_through_guest_memory() {
        // fnstcw word ptr [2000h]; fldcw word ptr [2000h]
        let code = [
            0xd9, 0x3d, 0x00, 0x20, 0x00, 0x00, 0xd9, 0x2d, 0x00, 0x20, 0x00, 0x00,
        ];
        let (mut cpu, mut memory) = machine(&code);
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(memory.read_u16(GuestAddress(0x2000)).unwrap(), 0x037f);
        memory.write_u16(GuestAddress(0x2000), 0x027f).unwrap();
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(cpu.state.x87_control_word, 0x027f);
    }

    #[test]
    fn fnclex_clears_x87_exception_state_only() {
        let (mut cpu, mut memory) = machine(&[0xdb, 0xe2]);
        cpu.state.x87_status_word = 0xffff;
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(cpu.state.x87_status_word, 0x7f00);
        assert_eq!(cpu.state.x87_control_word, 0x037f);
    }

    #[test]
    fn arithmetic_flags_cover_unsigned_and_signed_overflow() {
        let add_result = 0x7fff_ffff_u32.wrapping_add(1);
        assert_ne!(
            add_flags(0x7fff_ffff, 1, add_result, OperandWidth::Dword) & FLAG_OF,
            0
        );
        assert_eq!(
            add_flags(0x7fff_ffff, 1, add_result, OperandWidth::Dword) & FLAG_CF,
            0
        );

        let wrap_result = u32::MAX.wrapping_add(1);
        let flags = add_flags(u32::MAX, 1, wrap_result, OperandWidth::Dword);
        assert_ne!(flags & FLAG_CF, 0);
        assert_ne!(flags & FLAG_ZF, 0);
    }

    #[test]
    fn executes_partial_register_extensions_and_shift() {
        // mov eax,12345678h; mov al,ABh; mov ah,CDh; movzx ecx,al;
        // movsx edx,ah; shl ecx,3
        let code = [
            0xb8, 0x78, 0x56, 0x34, 0x12, 0xb0, 0xab, 0xb4, 0xcd, 0x0f, 0xb6, 0xc8, 0x0f, 0xbe,
            0xd4, 0xc1, 0xe1, 0x03,
        ];
        let (mut cpu, mut memory) = machine(&code);
        for _ in 0..6 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }
        assert_eq!(cpu.state.registers.eax, 0x1234_cdab);
        assert_eq!(cpu.state.registers.ecx, 0x558);
        assert_eq!(cpu.state.registers.edx, 0xffff_ffcd);
    }

    #[test]
    fn rotates_byte_operands_and_sets_carry() {
        // mov al,81h; ror al,1; rol al,1
        let code = [0xb0, 0x81, 0xd0, 0xc8, 0xd0, 0xc0];
        let (mut cpu, mut memory) = machine(&code);
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(cpu.state.registers.eax & 0xff, 0xc0);
        assert!(cpu.state.registers.flag(FLAG_CF));
        assert!(!cpu.state.registers.flag(FLAG_OF));

        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(cpu.state.registers.eax & 0xff, 0x81);
        assert!(cpu.state.registers.flag(FLAG_CF));
        assert!(!cpu.state.registers.flag(FLAG_OF));
    }

    #[test]
    fn byte_addition_preserves_upper_register_and_sets_flags() {
        // mov eax,12340000h; mov al,FFh; add al,1
        let code = [0xb8, 0, 0, 0x34, 0x12, 0xb0, 0xff, 0x04, 1];
        let (mut cpu, mut memory) = machine(&code);
        for _ in 0..3 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }
        assert_eq!(cpu.state.registers.eax, 0x1234_0000);
        assert!(cpu.state.registers.flag(FLAG_CF));
        assert!(cpu.state.registers.flag(FLAG_ZF));
    }

    #[test]
    fn not_preserves_flags_and_partial_register_bits() {
        // mov eax,123456f0h; not al
        let code = [0xb8, 0xf0, 0x56, 0x34, 0x12, 0xf6, 0xd0];
        let (mut cpu, mut memory) = machine(&code);
        cpu.state.registers.eflags = 0x247;
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(cpu.state.registers.eax, 0x1234_560f);
        assert_eq!(cpu.state.registers.eflags, 0x247);
    }

    #[test]
    fn xchg_swaps_partial_registers_without_changing_flags() {
        // mov eax,12345678h; xchg al,ah
        let code = [0xb8, 0x78, 0x56, 0x34, 0x12, 0x86, 0xe0];
        let (mut cpu, mut memory) = machine(&code);
        cpu.state.registers.eflags = 0x246;
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(cpu.state.registers.eax, 0x1234_7856);
        assert_eq!(cpu.state.registers.eflags, 0x246);
    }

    #[test]
    fn observed_mmx_fill_sequence_writes_paired_dwords() {
        // mov eax,50; movd mm0,eax; punpckldq mm0,mm0;
        // movq qword ptr [2000h],mm0; emms
        let code = [
            0xb8, 50, 0, 0, 0, 0x0f, 0x6e, 0xc0, 0x0f, 0x62, 0xc0, 0x0f, 0x7f, 0x05, 0x00, 0x20,
            0x00, 0x00, 0x0f, 0x77,
        ];
        let (mut cpu, mut memory) = machine(&code);
        for _ in 0..5 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }
        assert_eq!(memory.read_u32(GuestAddress(0x2000)).unwrap(), 50);
        assert_eq!(memory.read_u32(GuestAddress(0x2004)).unwrap(), 50);
    }

    #[test]
    fn adc_and_sbb_consume_and_replace_carry() {
        // mov eax,-1; xor ecx,ecx; sub ecx,1; adc eax,0;
        // mov ebx,0; sbb ebx,0
        let code = [
            0xb8, 0xff, 0xff, 0xff, 0xff, 0x31, 0xc9, 0x83, 0xe9, 0x01, 0x83, 0xd0, 0x00, 0xbb,
            0x00, 0x00, 0x00, 0x00, 0x83, 0xdb, 0x00,
        ];
        let (mut cpu, mut memory) = machine(&code);
        for _ in 0..6 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }
        assert_eq!(cpu.state.registers.eax, 0);
        assert_eq!(cpu.state.registers.ebx, u32::MAX);
        assert!(cpu.state.registers.flag(FLAG_CF));
        assert!(cpu.state.registers.flag(FLAG_SF));
    }

    #[test]
    fn fs_override_uses_the_configured_linear_base() {
        // mov eax,fs:[18h]; mov fs:[34h],eax
        let code = [
            0x64, 0xa1, 0x18, 0x00, 0x00, 0x00, 0x64, 0xa3, 0x34, 0x00, 0x00, 0x00,
        ];
        let (mut cpu, mut memory) = machine(&code);
        memory
            .map_range(GuestAddress(0x5000), 0x1000, Permissions::READ_WRITE)
            .unwrap();
        memory.write_u32(GuestAddress(0x5018), 0x1234_5678).unwrap();
        cpu.state.fs_base = 0x5000;

        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        cpu.step(&mut memory, &NoExternalTargets).unwrap();

        assert_eq!(cpu.state.registers.eax, 0x1234_5678);
        assert_eq!(memory.read_u32(GuestAddress(0x5034)).unwrap(), 0x1234_5678);
    }

    #[test]
    fn executes_setcc_and_conditional_move() {
        // mov eax,1; cmp eax,1; sete cl; setne dl; mov ebx,42; cmove eax,ebx
        let code = [
            0xb8, 1, 0, 0, 0, 0x83, 0xf8, 1, 0x0f, 0x94, 0xc1, 0x0f, 0x95, 0xc2, 0xbb, 42, 0, 0, 0,
            0x0f, 0x44, 0xc3,
        ];
        let (mut cpu, mut memory) = machine(&code);
        for _ in 0..6 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }

        assert_eq!(cpu.state.registers.eax, 42);
        assert_eq!(cpu.state.registers.ecx & 0xff, 1);
        assert_eq!(cpu.state.registers.edx & 0xff, 0);
        assert!(cpu.state.registers.flag(FLAG_ZF));
    }

    #[test]
    fn executes_variable_multiply_divide_and_right_shifts() {
        // mov eax,1234567; mov ebx,eax; mov ecx,37; imul eax,ecx;
        // xor edx,edx; div ecx; mov cl,5; shr ebx,cl
        let code = [
            0xb8, 0x87, 0xd6, 0x12, 0x00, 0x89, 0xc3, 0xb9, 37, 0, 0, 0, 0x0f, 0xaf, 0xc1, 0x31,
            0xd2, 0xf7, 0xf1, 0xb1, 5, 0xd3, 0xeb,
        ];
        let (mut cpu, mut memory) = machine(&code);
        for _ in 0..8 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }

        assert_eq!(cpu.state.registers.eax, 1_234_567);
        assert_eq!(cpu.state.registers.edx, 0);
        assert_eq!(cpu.state.registers.ebx, 38_580);
    }

    #[test]
    fn executes_signed_divide_arithmetic_shift_and_negate() {
        // mov eax,-1234567; mov ebx,eax; cdq; mov ecx,37; idiv ecx;
        // mov esi,eax; neg esi; mov cl,5; sar ebx,cl
        let code = [
            0xb8, 0x79, 0x29, 0xed, 0xff, 0x89, 0xc3, 0x99, 0xb9, 37, 0, 0, 0, 0xf7, 0xf9, 0x89,
            0xc6, 0xf7, 0xde, 0xb1, 5, 0xd3, 0xfb,
        ];
        let (mut cpu, mut memory) = machine(&code);
        for _ in 0..9 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }

        assert_eq!(cpu.state.registers.eax as i32, -33_366);
        assert_eq!(cpu.state.registers.edx as i32, -25);
        assert_eq!(cpu.state.registers.esi, 33_366);
        assert_eq!(cpu.state.registers.ebx as i32, -38_581);
    }

    #[test]
    fn implicit_multiply_and_divide_faults_are_explicit() {
        // mov eax,-1; mov ecx,2; mul ecx
        let code = [0xb8, 0xff, 0xff, 0xff, 0xff, 0xb9, 2, 0, 0, 0, 0xf7, 0xe1];
        let (mut cpu, mut memory) = machine(&code);
        for _ in 0..3 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }
        assert_eq!(cpu.state.registers.eax, 0xffff_fffe);
        assert_eq!(cpu.state.registers.edx, 1);
        assert!(cpu.state.registers.flag(FLAG_CF));
        assert!(cpu.state.registers.flag(FLAG_OF));

        // xor ecx,ecx; xor edx,edx; div ecx
        let (mut cpu, mut memory) = machine(&[0x31, 0xc9, 0x31, 0xd2, 0xf7, 0xf1]);
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        let error = cpu
            .step(&mut memory, &NoExternalTargets)
            .expect_err("division by zero must fault");
        assert!(matches!(
            error,
            CpuError::DivideError {
                detail: "division by zero",
                ..
            }
        ));
    }

    #[test]
    fn rep_movsb_obeys_direction_and_instruction_budgeting() {
        // cld; rep movsb
        let (mut cpu, mut memory) = machine(&[0xfc, 0xf3, 0xa4]);
        memory.write(GuestAddress(0x2000), &[1, 2, 3, 4]).unwrap();
        cpu.state.registers.esi = 0x2000;
        cpu.state.registers.edi = 0x2020;
        cpu.state.registers.ecx = 4;

        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        for remaining in (0..4).rev() {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
            assert_eq!(cpu.state.registers.ecx, remaining);
            if remaining != 0 {
                assert_eq!(cpu.state.registers.eip, 0x1001);
            }
        }
        let mut copied = [0; 4];
        memory.read(GuestAddress(0x2020), &mut copied).unwrap();
        assert_eq!(copied, [1, 2, 3, 4]);
        assert_eq!(cpu.state.registers.esi, 0x2004);
        assert_eq!(cpu.state.registers.edi, 0x2024);
        assert_eq!(cpu.state.registers.eip, 0x1003);

        // std; rep movsb; cld
        let (mut cpu, mut memory) = machine(&[0xfd, 0xf3, 0xa4, 0xfc]);
        memory.write(GuestAddress(0x2000), &[5, 6, 7, 8]).unwrap();
        cpu.state.registers.esi = 0x2003;
        cpu.state.registers.edi = 0x2023;
        cpu.state.registers.ecx = 4;
        for _ in 0..6 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }
        memory.read(GuestAddress(0x2020), &mut copied).unwrap();
        assert_eq!(copied, [5, 6, 7, 8]);
        assert_eq!(cpu.state.registers.esi, 0x1fff);
        assert_eq!(cpu.state.registers.edi, 0x201f);
        assert!(!cpu.state.registers.flag(FLAG_DF));
    }

    #[test]
    fn repe_cmps_and_repne_scas_stop_on_zf() {
        // cld; repe cmpsb
        let (mut cpu, mut memory) = machine(&[0xfc, 0xf3, 0xa6]);
        memory.write(GuestAddress(0x2000), &[1, 2, 9, 4]).unwrap();
        memory.write(GuestAddress(0x2020), &[1, 2, 3, 4]).unwrap();
        cpu.state.registers.esi = 0x2000;
        cpu.state.registers.edi = 0x2020;
        cpu.state.registers.ecx = 4;
        for _ in 0..4 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }
        assert_eq!(cpu.state.registers.ecx, 1);
        assert_eq!(cpu.state.registers.esi, 0x2003);
        assert_eq!(cpu.state.registers.edi, 0x2023);
        assert!(!cpu.state.registers.flag(FLAG_ZF));

        // cld; repne scasb
        let (mut cpu, mut memory) = machine(&[0xfc, 0xf2, 0xae]);
        memory.write(GuestAddress(0x2020), &[1, 2, 3, 4]).unwrap();
        cpu.state.registers.eax = 3;
        cpu.state.registers.edi = 0x2020;
        cpu.state.registers.ecx = 4;
        for _ in 0..4 {
            cpu.step(&mut memory, &NoExternalTargets).unwrap();
        }
        assert_eq!(cpu.state.registers.ecx, 1);
        assert_eq!(cpu.state.registers.edi, 0x2023);
        assert!(cpu.state.registers.flag(FLAG_ZF));
    }

    #[test]
    fn rep_stosd_and_lodsd_use_the_accumulator() {
        // cld; rep stosd; lodsd
        let (mut cpu, mut memory) = machine(&[0xfc, 0xf3, 0xab, 0xad]);
        cpu.state.registers.eax = 0x1234_5678;
        cpu.state.registers.edi = 0x2020;
        cpu.state.registers.ecx = 2;
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(memory.read_u32(GuestAddress(0x2020)).unwrap(), 0x1234_5678);
        assert_eq!(memory.read_u32(GuestAddress(0x2024)).unwrap(), 0x1234_5678);

        cpu.state.registers.eax = 0;
        cpu.state.registers.esi = 0x2024;
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(cpu.state.registers.eax, 0x1234_5678);
        assert_eq!(cpu.state.registers.esi, 0x2028);
    }

    #[test]
    fn decode_cache_observes_self_modifying_code() {
        let (mut cpu, mut memory) = machine(&[0xb8, 1, 0, 0, 0]);
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(cpu.state.registers.eax, 1);

        memory
            .write(GuestAddress(0x1001), &2_u32.to_le_bytes())
            .unwrap();
        cpu.state.registers.eip = 0x1000;
        cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(cpu.state.registers.eax, 2);
    }

    #[test]
    fn block_cache_stops_before_modified_following_instruction() {
        let code = [
            0xc6, 0x05, 0x08, 0x10, 0x00, 0x00, 0x02, // mov byte ptr [1008h], 2
            0xb8, 0x01, 0x00, 0x00, 0x00, // mov eax, 1
            0xf4, // hlt
        ];
        let (mut cpu, mut memory) = machine(&code);
        let outcome = cpu.run_batch(&mut memory, &NoExternalTargets, 16).unwrap();
        assert!(matches!(outcome, BatchOutcome::Halted { .. }));
        assert_eq!(cpu.state.registers.eax, 2);
    }

    #[test]
    fn reports_int3_as_a_breakpoint_exception() {
        let (mut cpu, mut memory) = machine(&[0xcc]);
        let outcome = cpu.step(&mut memory, &NoExternalTargets).unwrap();
        assert_eq!(
            outcome,
            StepOutcome::Exception {
                exception: CpuException::Breakpoint {
                    address: GuestAddress(0x1000),
                    resume_address: GuestAddress(0x1001),
                },
            }
        );
        assert_eq!(cpu.state.registers.eip, 0x1001);
    }

    fn machine(code: &[u8]) -> (Interpreter, GuestMemory) {
        let mut memory = GuestMemory::new();
        memory
            .map_range(GuestAddress(0x1000), 0x1000, Permissions::ALL)
            .unwrap();
        memory
            .map_range(GuestAddress(0x2000), 0x1000, Permissions::READ_WRITE)
            .unwrap();
        memory.write(GuestAddress(0x1000), code).unwrap();
        let mut cpu = Interpreter::new(GuestAddress(0x1000));
        cpu.state.registers.esp = 0x3000;
        (cpu, memory)
    }
}
