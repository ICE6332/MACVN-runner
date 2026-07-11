//! Target-driven `dsound.dll` compatibility surface.

use vnrt_win32::{ApiKey, ApiRegistry, GuestAddress, HostCallContext, HostCallHandler, Win32Error};

const MODULE: &str = "dsound.dll";
const DSERR_NODRIVER: u32 = 0x8878_0078;

/// Register the DirectSound APIs currently required by the selected Guest.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "DirectSoundCreate"), DirectSoundCreate);
    registry.register(ApiKey::new(MODULE, "DirectSoundCreate8"), DirectSoundCreate);
    registry.register(
        ApiKey::new(MODULE, "DirectSoundEnumerateA"),
        DirectSoundEnumerate,
    );
    registry.register(
        ApiKey::new(MODULE, "DirectSoundEnumerateW"),
        DirectSoundEnumerate,
    );
}

#[derive(Debug, Clone, Copy)]
struct DirectSoundCreate;

#[derive(Debug, Clone, Copy)]
struct DirectSoundEnumerate;

impl HostCallHandler for DirectSoundCreate {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _device_guid = context.argument_u32(0)?;
        let output = GuestAddress(context.argument_u32(1)?);
        let _outer = context.argument_u32(2)?;
        if output.0 != 0 {
            context.write_memory(output, &0_u32.to_le_bytes())?;
        }
        // Stay consistent with WinMM's zero-device report until audio gains a
        // real Host backend; never return a COM object that cannot play data.
        context.set_return_u32(DSERR_NODRIVER);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for DirectSoundEnumerate {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _callback = context.argument_u32(0)?;
        let _user = context.argument_u32(1)?;
        context.set_return_u32(0); // DS_OK, with no enumerated devices.
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_direct_sound_probe_surface() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 4);
    }
}
