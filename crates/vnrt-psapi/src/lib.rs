//! Target-driven Psapi compatibility handlers.

use vnrt_win32::{
    ApiKey, ApiRegistry, GuestAddress, HostCallContext, HostCallHandler, Win32Error, encode_ansi_z,
    encode_utf16_z,
};

const MODULE: &str = "psapi.dll";

/// Register the currently executed Psapi surface.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(
        ApiKey::new(MODULE, "GetModuleBaseNameA"),
        GetModuleBaseName { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetModuleBaseNameW"),
        GetModuleBaseName { wide: true },
    );
}

#[derive(Debug, Clone, Copy)]
struct GetModuleBaseName {
    wide: bool,
}

impl HostCallHandler for GetModuleBaseName {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        if context.argument_u32(0)? != u32::MAX {
            return Err(Win32Error::Unsupported {
                feature: "GetModuleBaseName for a non-current process",
            });
        }
        let module = GuestAddress(context.argument_u32(1)?);
        let name = if module.0 == 0 || module == context.main_module_base() {
            context
                .main_module_path()
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(context.main_module_path())
                .to_owned()
        } else if let Some(name) = context.loaded_module_name(module) {
            name
        } else {
            context.set_last_error(126); // ERROR_MOD_NOT_FOUND
            context.set_return_u32(0);
            context.set_stdcall_cleanup(16);
            return Ok(());
        };
        let capacity =
            usize::try_from(context.argument_u32(3)?).map_err(|_| Win32Error::OutOfMemory)?;
        if capacity == 0 {
            context.set_last_error(122); // ERROR_INSUFFICIENT_BUFFER
            context.set_return_u32(0);
            context.set_stdcall_cleanup(16);
            return Ok(());
        }
        let output = GuestAddress(context.argument_u32(2)?);
        let (mut encoded, stride) = if self.wide {
            (encode_utf16_z(&name), 2)
        } else {
            (encode_ansi_z(&name), 1)
        };
        let capacity_bytes = capacity
            .checked_mul(stride)
            .ok_or(Win32Error::OutOfMemory)?;
        encoded.truncate(capacity_bytes);
        if encoded.len() >= stride {
            let end = encoded.len();
            encoded[end - stride..].fill(0);
        }
        context.write_memory(output, &encoded)?;
        let copied = encoded.len() / stride - 1;
        context.set_last_error(0);
        context.set_return_u32(u32::try_from(copied).map_err(|_| Win32Error::OutOfMemory)?);
        context.set_stdcall_cleanup(16);
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
                .resolve(&ApiKey::new(MODULE, "GetModuleBaseNameA"))
                .is_some()
        );
    }
}
