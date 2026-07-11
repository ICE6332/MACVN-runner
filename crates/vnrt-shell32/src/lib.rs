//! Target-driven `shell32.dll` host-call surface.

use vnrt_win32::{
    ApiKey, ApiRegistry, GuestAddress, HostCallContext, HostCallHandler, Win32Error, encode_ansi_z,
};

const MODULE: &str = "shell32.dll";
const CSIDL_PERSONAL: u32 = 0x0005;
const CSIDL_APPDATA: u32 = 0x001a;
const CSIDL_LOCAL_APPDATA: u32 = 0x001c;

/// Register the Shell APIs currently required by the selected Guest.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(
        ApiKey::new(MODULE, "SHGetSpecialFolderPathA"),
        ShGetSpecialFolderPathA,
    );
}

#[derive(Debug, Clone, Copy)]
struct ShGetSpecialFolderPathA;

impl HostCallHandler for ShGetSpecialFolderPathA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        if context.argument_u32(0)? != 0 {
            return Err(Win32Error::Unsupported {
                feature: "SHGetSpecialFolderPathA owner window",
            });
        }
        let output = GuestAddress(context.argument_u32(1)?);
        let folder = context.argument_u32(2)?;
        let path = match folder {
            CSIDL_PERSONAL => r"C:\Users\VNRT\Documents",
            CSIDL_APPDATA => r"C:\Users\VNRT\AppData\Roaming",
            CSIDL_LOCAL_APPDATA => r"C:\Users\VNRT\AppData\Local",
            _ => {
                return Err(Win32Error::Unsupported {
                    feature: "SHGetSpecialFolderPathA CSIDL",
                });
            }
        };
        let _create = context.argument_u32(3)?;
        context.write_memory(output, &encode_ansi_z(path))?;
        context.set_return_u32(1);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_appdata_lookup() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        assert!(
            registry
                .resolve(&ApiKey::new(MODULE, "SHGetSpecialFolderPathA"))
                .is_some()
        );
    }
}
