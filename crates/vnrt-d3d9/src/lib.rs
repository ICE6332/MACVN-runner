//! Target-driven `d3d9.dll` graphics compatibility surface.

use vnrt_win32::{ApiKey, ApiRegistry, HostCallContext, HostCallHandler, Win32Error};

const MODULE: &str = "d3d9.dll";

/// Register the Direct3D 9 APIs currently required by the selected Guest.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "Direct3DCreate9"), Direct3DCreate9);
}

#[derive(Debug, Clone, Copy)]
struct Direct3DCreate9;

impl HostCallHandler for Direct3DCreate9 {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _sdk_version = context.argument_u32(0)?;
        // The export is present so module discovery succeeds. Returning null
        // keeps device creation explicitly unavailable until the target proves
        // it needs the IDirect3D9 object and its COM vtable.
        context.set_return_u32(0);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_factory() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 1);
    }
}
