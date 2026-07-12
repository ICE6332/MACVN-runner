//! Target-driven `comctl32.dll` common-controls compatibility surface.

use vnrt_win32::{ApiKey, ApiRegistry, GuestAddress, HostCallContext, HostCallHandler, Win32Error};

const MODULE: &str = "comctl32.dll";

/// Register the common-controls APIs currently required by the selected Guest.
pub fn register(registry: &mut ApiRegistry) {
    // Legacy applications commonly import InitCommonControls by its stable
    // COMCTL32 ordinal instead of by name. Keep both identities backed by the
    // same stateless implementation so synthetic PE exports can expose #17.
    registry.register(ApiKey::new(MODULE, "#17"), InitCommonControls);
    registry.register(
        ApiKey::new(MODULE, "InitCommonControls"),
        InitCommonControls,
    );
    registry.register(
        ApiKey::new(MODULE, "InitCommonControlsEx"),
        InitCommonControlsEx,
    );
}

#[derive(Debug, Clone, Copy)]
struct InitCommonControls;

#[derive(Debug, Clone, Copy)]
struct InitCommonControlsEx;

impl HostCallHandler for InitCommonControls {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

impl HostCallHandler for InitCommonControlsEx {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let input = GuestAddress(context.argument_u32(0)?);
        let mut controls = [0; 8];
        context.read_memory(input, &mut controls)?;
        let size = u32::from_le_bytes(
            controls[0..4]
                .try_into()
                .expect("INITCOMMONCONTROLSEX size"),
        );
        context.set_return_u32(u32::from(size == 8));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_common_control_initializers() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 3);
        assert!(registry.resolve(&ApiKey::new(MODULE, "#17")).is_some());
    }
}
