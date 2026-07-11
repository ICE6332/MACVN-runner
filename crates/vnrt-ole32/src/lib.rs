//! Minimal `ole32.dll` apartment lifecycle used by Guest startup.

use vnrt_win32::{ApiKey, ApiRegistry, GuestAddress, HostCallContext, HostCallHandler, Win32Error};

const MODULE: &str = "ole32.dll";

/// Register the COM APIs currently required by the selected Guest.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "CoInitialize"), CoInitialize);
    registry.register(ApiKey::new(MODULE, "CoInitializeEx"), CoInitializeEx);
    registry.register(ApiKey::new(MODULE, "CoUninitialize"), CoUninitialize);
    registry.register(ApiKey::new(MODULE, "CoCreateInstance"), CoCreateInstance);
}

#[derive(Debug, Clone, Copy)]
struct CoInitialize;

#[derive(Debug, Clone, Copy)]
struct CoInitializeEx;

#[derive(Debug, Clone, Copy)]
struct CoCreateInstance;

impl HostCallHandler for CoInitializeEx {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let reserved = context.argument_u32(0)?;
        let _model = context.argument_u32(1)?;
        if reserved != 0 {
            context.set_return_u32(0x8007_0057); // E_INVALIDARG
        } else {
            let result = context.initialize_com();
            context.set_return_u32(result);
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for CoCreateInstance {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let class_id = GuestAddress(context.argument_u32(0)?);
        let _outer = context.argument_u32(1)?;
        let _class_context = context.argument_u32(2)?;
        let interface_id = GuestAddress(context.argument_u32(3)?);
        let output = GuestAddress(context.argument_u32(4)?);
        let mut identifier = [0; 16];
        context.read_memory(class_id, &mut identifier)?;
        context.read_memory(interface_id, &mut identifier)?;
        context.write_memory(output, &0_u32.to_le_bytes())?;
        // Unknown COM classes fail through the documented registry lookup so
        // callers can use their non-COM fallback without a fake interface.
        context.set_return_u32(0x8004_0154); // REGDB_E_CLASSNOTREG
        context.set_stdcall_cleanup(20);
        Ok(())
    }
}

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
        assert_eq!(registry.len(), 4);
    }
}
