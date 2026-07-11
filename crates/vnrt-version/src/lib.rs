//! Target-driven `version.dll` version-resource compatibility surface.

use vnrt_win32::{ApiKey, ApiRegistry, GuestAddress, HostCallContext, HostCallHandler, Win32Error};

const MODULE: &str = "version.dll";

/// Register the version-resource APIs currently required by the selected Guest.
pub fn register(registry: &mut ApiRegistry) {
    for name in ["GetFileVersionInfoSizeA", "GetFileVersionInfoSizeW"] {
        registry.register(ApiKey::new(MODULE, name), VersionInfoSize);
    }
    for name in ["GetFileVersionInfoA", "GetFileVersionInfoW"] {
        registry.register(ApiKey::new(MODULE, name), VersionInfoUnavailable);
    }
    for name in ["VerQueryValueA", "VerQueryValueW"] {
        registry.register(ApiKey::new(MODULE, name), QueryValueUnavailable);
    }
}

#[derive(Debug, Clone, Copy)]
struct VersionInfoSize;

#[derive(Debug, Clone, Copy)]
struct VersionInfoUnavailable;

#[derive(Debug, Clone, Copy)]
struct QueryValueUnavailable;

impl HostCallHandler for VersionInfoSize {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _path = context.argument_u32(0)?;
        let handle_output = GuestAddress(context.argument_u32(1)?);
        if handle_output.0 != 0 {
            context.write_memory(handle_output, &0_u32.to_le_bytes())?;
        }
        context.set_last_error(1813); // ERROR_RESOURCE_TYPE_NOT_FOUND
        context.set_return_u32(0);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for VersionInfoUnavailable {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        for index in 0..4 {
            let _ = context.argument_u32(index)?;
        }
        context.set_last_error(1813); // ERROR_RESOURCE_TYPE_NOT_FOUND
        context.set_return_u32(0);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

impl HostCallHandler for QueryValueUnavailable {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _block = context.argument_u32(0)?;
        let _sub_block = context.argument_u32(1)?;
        let value_output = GuestAddress(context.argument_u32(2)?);
        let length_output = GuestAddress(context.argument_u32(3)?);
        if value_output.0 != 0 {
            context.write_memory(value_output, &0_u32.to_le_bytes())?;
        }
        if length_output.0 != 0 {
            context.write_memory(length_output, &0_u32.to_le_bytes())?;
        }
        context.set_return_u32(0);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_version_resource_surface() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 6);
    }
}
