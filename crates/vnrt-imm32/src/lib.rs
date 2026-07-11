//! Target-driven `imm32.dll` input-method compatibility surface.

use vnrt_win32::{ApiKey, ApiRegistry, HostCallContext, HostCallHandler, Win32Error};

const MODULE: &str = "imm32.dll";

/// Register the input-method APIs currently required by the selected Guest.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "ImmGetContext"), ImmGetContext);
    register_constant(registry, "ImmReleaseContext", 8, 1);
    register_constant(registry, "ImmAssociateContext", 8, 0);
    register_constant(registry, "ImmGetOpenStatus", 4, 0);
    register_constant(registry, "ImmSetOpenStatus", 8, 1);
    register_constant(registry, "ImmGetCompositionStringA", 16, 0);
    register_constant(registry, "ImmGetCompositionStringW", 16, 0);
    register_constant(registry, "ImmSetCompositionWindow", 8, 1);
    register_constant(registry, "ImmSetCandidateWindow", 8, 1);
    register_constant(registry, "ImmNotifyIME", 16, 1);
}

fn register_constant(registry: &mut ApiRegistry, name: &str, cleanup: u32, result: u32) {
    registry.register(
        ApiKey::new(MODULE, name),
        ConstantResult { cleanup, result },
    );
}

#[derive(Debug, Clone, Copy)]
struct ImmGetContext;

#[derive(Debug, Clone, Copy)]
struct ConstantResult {
    cleanup: u32,
    result: u32,
}

impl HostCallHandler for ImmGetContext {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        // A stable window-derived value preserves Get/Release pairing while
        // composition is disabled and no Host input-method object is exposed.
        context.set_return_u32(if window == 0 { 0 } else { window });
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for ConstantResult {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        for index in 0..(self.cleanup / 4) as usize {
            let _ = context.argument_u32(index)?;
        }
        context.set_return_u32(self.result);
        context.set_stdcall_cleanup(self.cleanup);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_input_method_surface() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 10);
    }
}
