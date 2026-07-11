//! Target-observed NTDLL exports used by low-level loader code.

use vnrt_memory::GuestAddress;
use vnrt_win32::{
    ApiKey, ApiRegistry, HostCallContext, HostCallHandler, UnsupportedApi, Win32Error,
};

const MODULE: &str = "ntdll.dll";

/// Register the currently observed NTDLL compatibility surface.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(
        ApiKey::new(MODULE, "NtAllocateVirtualMemory"),
        NtAllocateVirtualMemory,
    );
    registry.register(ApiKey::new(MODULE, "RtlAcquirePebLock"), PebLock);
    registry.register(ApiKey::new(MODULE, "RtlReleasePebLock"), PebLock);
    registry.register(
        ApiKey::new(MODULE, "RtlInitUnicodeString"),
        RtlInitUnicodeString,
    );
    registry.register(
        ApiKey::new(MODULE, "NtReadVirtualMemory"),
        VirtualMemoryCopy { write: false },
    );
    registry.register(
        ApiKey::new(MODULE, "NtWriteVirtualMemory"),
        VirtualMemoryCopy { write: true },
    );
    for (name, feature) in [
        ("CsrClientCallServer", "CSR client/server call"),
        ("NtContinue", "NtContinue context restoration"),
        ("NtClose", "NT handle close"),
        ("NtCreateKey", "NT registry key creation"),
        ("NtCreateFile", "NT file creation/opening"),
        ("NtCreateSection", "NT section creation"),
        ("NtDeleteKey", "NT registry key deletion"),
        ("NtDeleteValueKey", "NT registry value deletion"),
        ("NtDuplicateObject", "NT handle duplication"),
        ("NtEnumerateKey", "NT registry key enumeration"),
        ("NtEnumerateValueKey", "NT registry value enumeration"),
        ("NtFlushInstructionCache", "NT instruction cache flush"),
        ("NtFreeVirtualMemory", "NT virtual-memory release"),
        ("NtMapViewOfSection", "NT section view mapping"),
        ("NtOpenKey", "NT registry key opening"),
        ("NtOpenFile", "NT file opening"),
        ("NtOpenThread", "NT thread opening"),
        ("NtQueryKey", "NT registry key metadata query"),
        ("NtQueryAttributesFile", "NT file attribute query"),
        ("NtQueryFullAttributesFile", "NT full file attribute query"),
        ("NtQueryInformationProcess", "NT process information query"),
        ("NtQueryInformationFile", "NT file information query"),
        ("NtQueryInformationThread", "NT thread information query"),
        ("NtQueryObject", "NT object metadata query"),
        ("NtQuerySection", "NtQuerySection metadata"),
        ("NtQuerySystemInformation", "NT system information query"),
        ("NtQueryValueKey", "NT registry value query"),
        ("NtQueryVirtualMemory", "NT virtual-memory query"),
        ("NtProtectVirtualMemory", "NT virtual-memory protection"),
        ("NtReadFile", "NT file reading"),
        ("NtResumeThread", "NT thread resumption"),
        ("NtSetInformationProcess", "NT process information update"),
        ("NtSetInformationFile", "NT file information update"),
        ("NtSetInformationThread", "NT thread information update"),
        ("NtSetValueKey", "NT registry value update"),
        ("NtSuspendThread", "NT thread suspension"),
        ("NtTerminateThread", "NT thread termination"),
        ("NtUnmapViewOfSection", "NT section view unmapping"),
        ("NtWriteFile", "NT file writing"),
        ("RtlNtStatusToDosError", "NTSTATUS to Win32 error mapping"),
        (
            "RtlDosPathNameToNtPathName_U",
            "DOS path to NT path conversion",
        ),
        ("RtlFreeUnicodeString", "NT Unicode string release"),
        (
            "RtlExpandEnvironmentStrings_U",
            "NT environment string expansion",
        ),
        (
            "RtlQueryEnvironmentVariable_U",
            "NT environment variable query",
        ),
        (
            "RtlSetEnvironmentVariable",
            "NT environment variable update",
        ),
    ] {
        registry.register(ApiKey::new(MODULE, name), UnsupportedApi::new(feature));
    }
}

const MEM_COMMIT_RESERVE: u32 = 0x3000;

#[derive(Debug, Clone, Copy)]
struct NtAllocateVirtualMemory;

#[derive(Debug, Clone, Copy)]
struct PebLock;

#[derive(Debug, Clone, Copy)]
struct VirtualMemoryCopy {
    write: bool,
}

#[derive(Debug, Clone, Copy)]
struct RtlInitUnicodeString;

impl HostCallHandler for PebLock {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        // VNRT currently models one Guest thread, so the process-environment
        // lock cannot be contended. Keep the calls as an explicit no-op pair.
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

impl HostCallHandler for VirtualMemoryCopy {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        if context.argument_u32(0)? != u32::MAX {
            return Err(Win32Error::Unsupported {
                feature: "NT virtual-memory copy for a non-current process",
            });
        }
        let process_address = GuestAddress(context.argument_u32(1)?);
        let caller_buffer = GuestAddress(context.argument_u32(2)?);
        let byte_count =
            usize::try_from(context.argument_u32(3)?).map_err(|_| Win32Error::OutOfMemory)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(byte_count)
            .map_err(|_| Win32Error::OutOfMemory)?;
        bytes.resize(byte_count, 0);
        if self.write {
            context.read_memory(caller_buffer, &mut bytes)?;
            context.write_memory(process_address, &bytes)?;
        } else {
            context.read_memory(process_address, &mut bytes)?;
            context.write_memory(caller_buffer, &bytes)?;
        }
        let transferred_pointer = GuestAddress(context.argument_u32(4)?);
        if transferred_pointer.0 != 0 {
            let transferred = u32::try_from(byte_count).map_err(|_| Win32Error::OutOfMemory)?;
            context.write_memory(transferred_pointer, &transferred.to_le_bytes())?;
        }
        context.set_return_u32(0); // STATUS_SUCCESS
        context.set_stdcall_cleanup(20);
        Ok(())
    }
}

impl HostCallHandler for RtlInitUnicodeString {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let destination = GuestAddress(context.argument_u32(0)?);
        let source = GuestAddress(context.argument_u32(1)?);
        let (length, maximum_length, buffer) = if source.0 == 0 {
            (0_u16, 0_u16, 0_u32)
        } else {
            let mut units = 0_u32;
            loop {
                let byte_offset = units
                    .checked_mul(2)
                    .ok_or(Win32Error::InvalidArgument("Unicode string is too long"))?;
                let address =
                    source
                        .0
                        .checked_add(byte_offset)
                        .ok_or(Win32Error::InvalidArgument(
                            "Unicode string address overflow",
                        ))?;
                let mut bytes = [0; 2];
                context.read_memory(GuestAddress(address), &mut bytes)?;
                if u16::from_le_bytes(bytes) == 0 {
                    break;
                }
                units = units
                    .checked_add(1)
                    .ok_or(Win32Error::InvalidArgument("Unicode string is too long"))?;
                if units > 32_766 {
                    return Err(Win32Error::InvalidArgument("Unicode string is too long"));
                }
            }
            let byte_length = u16::try_from(units * 2)
                .map_err(|_| Win32Error::InvalidArgument("Unicode string is too long"))?;
            (byte_length, byte_length + 2, source.0)
        };
        let mut descriptor = [0; 8];
        descriptor[0..2].copy_from_slice(&length.to_le_bytes());
        descriptor[2..4].copy_from_slice(&maximum_length.to_le_bytes());
        descriptor[4..8].copy_from_slice(&buffer.to_le_bytes());
        context.write_memory(destination, &descriptor)?;
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for NtAllocateVirtualMemory {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        if context.argument_u32(0)? != u32::MAX {
            return Err(Win32Error::Unsupported {
                feature: "NtAllocateVirtualMemory for a non-current process",
            });
        }
        let base_address_pointer = GuestAddress(context.argument_u32(1)?);
        if context.argument_u32(2)? != 0 {
            return Err(Win32Error::Unsupported {
                feature: "NtAllocateVirtualMemory with nonzero ZeroBits",
            });
        }
        let region_size_pointer = GuestAddress(context.argument_u32(3)?);
        if context.argument_u32(4)? != MEM_COMMIT_RESERVE {
            return Err(Win32Error::Unsupported {
                feature: "NtAllocateVirtualMemory allocation type other than MEM_RESERVE|MEM_COMMIT",
            });
        }
        let requested_base = read_u32(context, base_address_pointer)?;
        if requested_base != 0 {
            return Err(Win32Error::Unsupported {
                feature: "NtAllocateVirtualMemory with a requested base address",
            });
        }
        let requested_size = read_u32(context, region_size_pointer)?;
        if requested_size == 0 {
            return Err(Win32Error::InvalidArgument(
                "NtAllocateVirtualMemory zero region size",
            ));
        }
        let (read, write, execute) = virtual_protection(context.argument_u32(5)?)?;
        let address = context.allocate_virtual_memory(requested_size, read, write, execute)?;
        context.write_memory(base_address_pointer, &address.0.to_le_bytes())?;
        context.write_memory(region_size_pointer, &requested_size.to_le_bytes())?;
        context.set_return_u32(0); // STATUS_SUCCESS
        context.set_stdcall_cleanup(24);
        Ok(())
    }
}

fn read_u32(context: &dyn HostCallContext, address: GuestAddress) -> Result<u32, Win32Error> {
    let mut bytes = [0; 4];
    context.read_memory(address, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn virtual_protection(protection: u32) -> Result<(bool, bool, bool), Win32Error> {
    match protection {
        0x01 => Ok((false, false, false)),
        0x02 => Ok((true, false, false)),
        0x04 => Ok((true, true, false)),
        0x10 => Ok((false, false, true)),
        0x20 => Ok((true, false, true)),
        0x40 => Ok((true, true, true)),
        _ => Err(Win32Error::Unsupported {
            feature: "NtAllocateVirtualMemory page protection flags",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_observed_loader_exports() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        for name in [
            "CsrClientCallServer",
            "NtContinue",
            "NtAllocateVirtualMemory",
            "NtClose",
            "NtCreateKey",
            "NtCreateFile",
            "NtCreateSection",
            "NtDeleteKey",
            "NtDeleteValueKey",
            "NtDuplicateObject",
            "NtEnumerateKey",
            "NtEnumerateValueKey",
            "NtFlushInstructionCache",
            "NtFreeVirtualMemory",
            "NtMapViewOfSection",
            "NtOpenKey",
            "NtOpenFile",
            "NtOpenThread",
            "NtQueryKey",
            "NtQueryAttributesFile",
            "NtQueryFullAttributesFile",
            "NtQueryInformationProcess",
            "NtQueryInformationFile",
            "NtQueryInformationThread",
            "NtQueryObject",
            "NtQuerySection",
            "NtQuerySystemInformation",
            "NtQueryValueKey",
            "NtQueryVirtualMemory",
            "NtProtectVirtualMemory",
            "NtReadFile",
            "NtReadVirtualMemory",
            "NtResumeThread",
            "NtSetInformationProcess",
            "NtSetInformationFile",
            "NtSetInformationThread",
            "NtSetValueKey",
            "NtSuspendThread",
            "NtTerminateThread",
            "NtUnmapViewOfSection",
            "NtWriteFile",
            "NtWriteVirtualMemory",
            "RtlNtStatusToDosError",
            "RtlDosPathNameToNtPathName_U",
            "RtlFreeUnicodeString",
            "RtlExpandEnvironmentStrings_U",
            "RtlQueryEnvironmentVariable_U",
            "RtlSetEnvironmentVariable",
            "RtlAcquirePebLock",
            "RtlInitUnicodeString",
            "RtlReleasePebLock",
        ] {
            assert!(registry.resolve(&ApiKey::new(MODULE, name)).is_some());
        }
    }
}
