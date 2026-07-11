//! Target-driven Advapi32 compatibility handlers.

use vnrt_win32::{
    ApiKey, ApiRegistry, GuestAddress, Handle, HostCallContext, HostCallHandler,
    PROCESS_HEAP_HANDLE, Win32Error, encode_utf16_z, read_ansi_z, read_utf16_z,
};

const MODULE: &str = "advapi32.dll";

/// Register the currently executed Advapi32 surface.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "OpenProcessToken"), OpenProcessToken);
    registry.register(
        ApiKey::new(MODULE, "GetTokenInformation"),
        GetTokenInformation,
    );
    registry.register(
        ApiKey::new(MODULE, "ConvertSidToStringSidW"),
        ConvertSidToStringSidW,
    );
    registry.register(ApiKey::new(MODULE, "RegCloseKey"), RegCloseKey);
    registry.register(
        ApiKey::new(MODULE, "RegOpenKeyExA"),
        RegOpenKeyEx {
            wide: false,
            extended: true,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "RegOpenKeyExW"),
        RegOpenKeyEx {
            wide: true,
            extended: true,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "RegOpenKeyA"),
        RegOpenKeyEx {
            wide: false,
            extended: false,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "RegOpenKeyW"),
        RegOpenKeyEx {
            wide: true,
            extended: false,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "RegQueryValueExA"),
        RegQueryValueEx { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "RegQueryValueExW"),
        RegQueryValueEx { wide: true },
    );
    registry.register(ApiKey::new(MODULE, "RegSetValueExA"), RegSetValueEx);
    registry.register(ApiKey::new(MODULE, "RegSetValueExW"), RegSetValueEx);
}

#[derive(Debug, Clone, Copy)]
struct OpenProcessToken;

#[derive(Debug, Clone, Copy)]
struct GetTokenInformation;

#[derive(Debug, Clone, Copy)]
struct ConvertSidToStringSidW;

#[derive(Debug, Clone, Copy)]
struct RegCloseKey;

#[derive(Debug, Clone, Copy)]
struct RegOpenKeyEx {
    wide: bool,
    extended: bool,
}

#[derive(Debug, Clone, Copy)]
struct RegQueryValueEx {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct RegSetValueEx;

impl HostCallHandler for RegOpenKeyEx {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _root = context.argument_u32(0)?;
        let subkey = GuestAddress(context.argument_u32(1)?);
        let (output, cleanup) = if self.extended {
            let _options = context.argument_u32(2)?;
            let _access = context.argument_u32(3)?;
            (GuestAddress(context.argument_u32(4)?), 20)
        } else {
            (GuestAddress(context.argument_u32(2)?), 12)
        };
        if subkey.0 != 0 {
            if self.wide {
                let _ = read_utf16_z(context, subkey)?;
            } else {
                let _ = read_ansi_z(context, subkey)?;
            }
        }
        if output.0 != 0 {
            context.write_memory(output, &0_u32.to_le_bytes())?;
        }
        // No Host registry is exposed; callers receive the normal missing-key
        // result and can follow their documented defaults.
        context.set_return_u32(2); // ERROR_FILE_NOT_FOUND
        context.set_stdcall_cleanup(cleanup);
        Ok(())
    }
}

impl HostCallHandler for RegQueryValueEx {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _key = context.argument_u32(0)?;
        let value_name = GuestAddress(context.argument_u32(1)?);
        let _reserved = context.argument_u32(2)?;
        let _type_output = context.argument_u32(3)?;
        let _data = context.argument_u32(4)?;
        let size_output = GuestAddress(context.argument_u32(5)?);
        if value_name.0 != 0 {
            if self.wide {
                let _ = read_utf16_z(context, value_name)?;
            } else {
                let _ = read_ansi_z(context, value_name)?;
            }
        }
        if size_output.0 != 0 {
            context.write_memory(size_output, &0_u32.to_le_bytes())?;
        }
        context.set_return_u32(2); // ERROR_FILE_NOT_FOUND
        context.set_stdcall_cleanup(24);
        Ok(())
    }
}

impl HostCallHandler for RegSetValueEx {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        for index in 0..6 {
            let _ = context.argument_u32(index)?;
        }
        context.set_return_u32(5); // ERROR_ACCESS_DENIED
        context.set_stdcall_cleanup(24);
        Ok(())
    }
}

impl HostCallHandler for RegCloseKey {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let key = context.argument_u32(0)?;
        context.set_return_u32(if key == 0 { 6 } else { 0 }); // ERROR_INVALID_HANDLE / SUCCESS
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for OpenProcessToken {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let process = Handle(context.argument_u32(0)?);
        let desired_access = context.argument_u32(1)?;
        let output = GuestAddress(context.argument_u32(2)?);
        match context.open_process_token(process, desired_access) {
            Ok(token) => {
                context.write_memory(output, &token.0.to_le_bytes())?;
                context.set_last_error(0);
                context.set_return_u32(1);
            }
            Err(Win32Error::InvalidHandle(_)) => {
                context.set_last_error(6); // ERROR_INVALID_HANDLE
                context.set_return_u32(0);
            }
            Err(error) => return Err(error),
        }
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for GetTokenInformation {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        const TOKEN_USER: u32 = 1;
        const REQUIRED_SIZE: u32 = 24;
        let token = Handle(context.argument_u32(0)?);
        if !context.token_is_open(token) {
            context.set_last_error(6); // ERROR_INVALID_HANDLE
            context.set_return_u32(0);
            context.set_stdcall_cleanup(20);
            return Ok(());
        }
        if context.argument_u32(1)? != TOKEN_USER {
            return Err(Win32Error::Unsupported {
                feature: "advapi32!GetTokenInformation class other than TokenUser",
            });
        }
        let output = GuestAddress(context.argument_u32(2)?);
        let output_size = context.argument_u32(3)?;
        let required_size_output = GuestAddress(context.argument_u32(4)?);
        if required_size_output.0 != 0 {
            context.write_memory(required_size_output, &REQUIRED_SIZE.to_le_bytes())?;
        }
        if output.0 == 0 || output_size < REQUIRED_SIZE {
            context.set_last_error(122); // ERROR_INSUFFICIENT_BUFFER
            context.set_return_u32(0);
            context.set_stdcall_cleanup(20);
            return Ok(());
        }

        let sid_address = output
            .0
            .checked_add(8)
            .ok_or(Win32Error::InvalidArgument("TOKEN_USER buffer overflow"))?;
        let mut token_user = [0_u8; REQUIRED_SIZE as usize];
        token_user[0..4].copy_from_slice(&sid_address.to_le_bytes());
        // Stable local user SID: S-1-5-21-1000.
        token_user[8..24].copy_from_slice(&[1, 2, 0, 0, 0, 0, 0, 5, 21, 0, 0, 0, 0xe8, 0x03, 0, 0]);
        context.write_memory(output, &token_user)?;
        context.set_last_error(0);
        context.set_return_u32(1);
        context.set_stdcall_cleanup(20);
        Ok(())
    }
}

impl HostCallHandler for ConvertSidToStringSidW {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let sid = GuestAddress(context.argument_u32(0)?);
        let output = GuestAddress(context.argument_u32(1)?);
        let mut header = [0_u8; 8];
        context.read_memory(sid, &mut header)?;
        let revision = header[0];
        let count = usize::from(header[1]);
        if revision == 0 || count > 15 {
            context.set_last_error(1337); // ERROR_INVALID_SID
            context.set_return_u32(0);
            context.set_stdcall_cleanup(8);
            return Ok(());
        }
        let authority = header[2..8]
            .iter()
            .fold(0_u64, |value, byte| (value << 8) | u64::from(*byte));
        let mut string = format!("S-{revision}-{authority}");
        for index in 0..count {
            let offset = u32::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(4))
                .and_then(|offset| offset.checked_add(8))
                .ok_or(Win32Error::InvalidArgument("SID sub-authority overflow"))?;
            let address = sid
                .0
                .checked_add(offset)
                .ok_or(Win32Error::InvalidArgument("SID address overflow"))?;
            let mut bytes = [0_u8; 4];
            context.read_memory(GuestAddress(address), &mut bytes)?;
            string.push('-');
            string.push_str(&u32::from_le_bytes(bytes).to_string());
        }
        let encoded = encode_utf16_z(&string);
        let size = u32::try_from(encoded.len()).map_err(|_| Win32Error::OutOfMemory)?;
        let allocation = context.allocate_heap_memory(Handle(PROCESS_HEAP_HANDLE), size)?;
        context.write_memory(allocation, &encoded)?;
        context.write_memory(output, &allocation.0.to_le_bytes())?;
        context.set_last_error(0);
        context.set_return_u32(1);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_initial_surface() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        assert!(
            registry
                .resolve(&ApiKey::new(MODULE, "OpenProcessToken"))
                .is_some()
        );
        assert!(
            registry
                .resolve(&ApiKey::new(MODULE, "GetTokenInformation"))
                .is_some()
        );
        assert!(
            registry
                .resolve(&ApiKey::new(MODULE, "ConvertSidToStringSidW"))
                .is_some()
        );
    }
}
