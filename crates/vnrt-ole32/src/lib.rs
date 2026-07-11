//! Minimal `ole32.dll` apartment lifecycle used by Guest startup.

use vnrt_win32::{ApiKey, ApiRegistry, HostCallContext, HostCallHandler, Win32Error};

const MODULE: &str = "ole32.dll";

/// Register the COM APIs currently required by the selected Guest.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "CoInitialize"), CoInitialize);
    registry.register(ApiKey::new(MODULE, "CoUninitialize"), CoUninitialize);
}

#[derive(Debug, Clone, Copy)]
struct CoInitialize;

impl HostCallHandler for CoInitialize {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        if context.argument_u32(0)? != 0 {
            return Err(Win32Error::Unsupported {
                feature: "CoInitialize reserved pointer",
            });
        }
        let result = context.initialize_com();
        context.set_return_u32(result);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct CoUninitialize;

impl HostCallHandler for CoUninitialize {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.uninitialize_com();
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_apartment_lifecycle() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 2);
    }
}
