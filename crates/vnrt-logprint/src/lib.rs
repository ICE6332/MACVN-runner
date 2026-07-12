//! Discovery surface for the target's optional `logprint.dll` plugin.

use vnrt_win32::{ApiKey, ApiRegistry, HostCallContext, HostCallHandler, Win32Error};

const MODULE: &str = "logprint.dll";

/// Make the optional module discoverable while its target exports are censused.
pub fn register(registry: &mut ApiRegistry) {
    // The renderer-side diagnostic call passes thirteen 32-bit values and the
    // callee owns their cleanup. Treating it as cdecl leaks 52 bytes and makes
    // the enclosing drawing routine return through one of its local fields.
    registry.register(ApiKey::new(MODULE, "Test"), Test { cleanup: 13 * 4 });
    // The omitted debug plugin exposes a numbered family of cdecl probes.
    // Keep the replacement deliberately inert; cdecl callers retain stack
    // cleanup, so the same zero-argument Host thunk safely represents them.
    for suffix in 2..=64 {
        registry.register(
            ApiKey::new(MODULE, format!("Test{suffix}")),
            Test { cleanup: 0 },
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct Test {
    cleanup: u32,
}

impl HostCallHandler for Test {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.set_return_u32(0);
        context.set_stdcall_cleanup(self.cleanup);
        Ok(())
    }
}
