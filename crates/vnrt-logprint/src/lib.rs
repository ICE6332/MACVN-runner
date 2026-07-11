//! Discovery surface for the target's optional `logprint.dll` plugin.

use vnrt_win32::{ApiKey, ApiRegistry, HostCallContext, HostCallHandler, Win32Error};

const MODULE: &str = "logprint.dll";

/// Make the optional module discoverable while its target exports are censused.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "Test"), Test);
    // The omitted debug plugin exposes a numbered family of cdecl probes.
    // Keep the replacement deliberately inert; cdecl callers retain stack
    // cleanup, so the same zero-argument Host thunk safely represents them.
    for suffix in 2..=64 {
        registry.register(ApiKey::new(MODULE, format!("Test{suffix}")), Test);
    }
}

#[derive(Debug, Clone, Copy)]
struct Test;

impl HostCallHandler for Test {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.set_return_u32(0);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}
