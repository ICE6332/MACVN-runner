//! Trace records and human-readable machine-state formatting.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use vnrt_memory::{GuestAddress, GuestMemory, MemoryError};
use vnrt_win32::ApiKey;
use vnrt_x86::Registers;

/// One serializable debugger observation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TraceEvent {
    /// A decoded instruction at a guest address.
    Instruction {
        /// Instruction pointer.
        address: GuestAddress,
        /// Display text supplied by the execution frontend.
        text: String,
    },
    /// A host-call transition.
    ApiCall {
        /// Imported API name.
        api: ApiKey,
    },
}

/// In-memory trace sink suitable for tests and initial CLI output.
#[derive(Debug, Default)]
pub struct TraceRecorder {
    events: Vec<TraceEvent>,
}

impl TraceRecorder {
    /// Append an observation.
    pub fn record(&mut self, event: TraceEvent) {
        self.events.push(event);
    }

    /// View all observations in execution order.
    #[must_use]
    pub fn events(&self) -> &[TraceEvent] {
        &self.events
    }
}

/// Debugger inspection errors.
#[derive(Debug, Error)]
pub enum DebuggerError {
    /// Guest bytes could not be read.
    #[error(transparent)]
    Memory(#[from] MemoryError),
    /// A richer debugger operation is not implemented yet.
    #[error("unsupported debugger operation: {0}")]
    Unsupported(&'static str),
}

/// Format the general-purpose register set as one compact line.
#[must_use]
pub fn format_registers(registers: &Registers) -> String {
    format!(
        "EAX={:08X} EBX={:08X} ECX={:08X} EDX={:08X} \
         ESI={:08X} EDI={:08X} ESP={:08X} EBP={:08X} \
         EIP={:08X} EFLAGS={:08X}",
        registers.eax,
        registers.ebx,
        registers.ecx,
        registers.edx,
        registers.esi,
        registers.edi,
        registers.esp,
        registers.ebp,
        registers.eip,
        registers.eflags,
    )
}

/// Copy a guest memory range for hex or structure inspection.
pub fn dump_memory(
    memory: &GuestMemory,
    address: GuestAddress,
    len: usize,
) -> Result<Vec<u8>, DebuggerError> {
    let mut bytes = vec![0; len];
    memory.read(address, &mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_format_is_stable() {
        let registers = Registers {
            eax: 0x1234,
            ..Registers::default()
        };
        assert!(format_registers(&registers).starts_with("EAX=00001234"));
    }
}
