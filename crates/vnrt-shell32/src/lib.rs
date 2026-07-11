//! Target-driven `shell32.dll` host-call surface.

use vnrt_win32::{
    ApiKey, ApiRegistry, GuestAddress, HostCallContext, HostCallHandler, Win32Error, encode_ansi_z,
    encode_utf16_z, read_ansi_z, read_utf16_z,
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
    registry.register(ApiKey::new(MODULE, "DragQueryFileA"), DragQueryFile);
    registry.register(ApiKey::new(MODULE, "DragQueryFileW"), DragQueryFile);
    registry.register(ApiKey::new(MODULE, "DragFinish"), DragFinish);
    registry.register(ApiKey::new(MODULE, "DragAcceptFiles"), DragAcceptFiles);
    registry.register(ApiKey::new(MODULE, "Shell_NotifyIconA"), ShellNotifyIcon);
    registry.register(ApiKey::new(MODULE, "Shell_NotifyIconW"), ShellNotifyIcon);
    registry.register(
        ApiKey::new(MODULE, "FindExecutableA"),
        FindExecutable { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "FindExecutableW"),
        FindExecutable { wide: true },
    );
    registry.register(ApiKey::new(MODULE, "ShellExecuteExA"), ShellExecuteEx);
    registry.register(ApiKey::new(MODULE, "ShellExecuteExW"), ShellExecuteEx);
    registry.register(ApiKey::new(MODULE, "ShellExecuteA"), ShellExecute);
    registry.register(ApiKey::new(MODULE, "ShellExecuteW"), ShellExecute);
}

#[derive(Debug, Clone, Copy)]
struct DragQueryFile;

#[derive(Debug, Clone, Copy)]
struct DragFinish;

#[derive(Debug, Clone, Copy)]
struct DragAcceptFiles;

#[derive(Debug, Clone, Copy)]
struct ShellNotifyIcon;

#[derive(Debug, Clone, Copy)]
struct FindExecutable {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct ShellExecuteEx;

#[derive(Debug, Clone, Copy)]
struct ShellExecute;

impl HostCallHandler for ShellExecuteEx {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let info = GuestAddress(context.argument_u32(0)?);
        let mut size = [0; 4];
        context.read_memory(info, &mut size)?;
        let size = u32::from_le_bytes(size);
        context.set_last_error(if size >= 60 { 50 } else { 87 });
        context.set_return_u32(0);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for ShellExecute {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        for index in 0..6 {
            let _ = context.argument_u32(index)?;
        }
        context.set_last_error(50); // ERROR_NOT_SUPPORTED
        context.set_return_u32(31); // SE_ERR_NOASSOC
        context.set_stdcall_cleanup(24);
        Ok(())
    }
}

impl HostCallHandler for FindExecutable {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let file = GuestAddress(context.argument_u32(0)?);
        let _directory = context.argument_u32(1)?;
        let output = GuestAddress(context.argument_u32(2)?);
        let file_name = if self.wide {
            read_utf16_z(context, file)?
        } else {
            read_ansi_z(context, file)?
        };
        if file_name.is_empty() {
            context.set_return_u32(2); // SE_ERR_FNF
        } else {
            let executable = r"C:\Windows\System32\notepad.exe";
            let encoded = if self.wide {
                encode_utf16_z(executable)
            } else {
                encode_ansi_z(executable)
            };
            context.write_memory(output, &encoded)?;
            context.set_return_u32(33); // success is any value greater than 32
        }
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for ShellNotifyIcon {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let operation = context.argument_u32(0)?;
        let data = GuestAddress(context.argument_u32(1)?);
        let mut header = [0; 4];
        context.read_memory(data, &mut header)?;
        let size = u32::from_le_bytes(header);
        let valid = operation <= 4 && (88..=1024).contains(&size);
        // Tray icons have no native presentation in the current backend, but
        // their lifecycle is independent of the game window and can succeed.
        context.set_return_u32(u32::from(valid));
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for DragQueryFile {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _drop = context.argument_u32(0)?;
        let _index = context.argument_u32(1)?;
        let _output = context.argument_u32(2)?;
        let _capacity = context.argument_u32(3)?;
        // The native backend has not delivered a drop object, so the modeled
        // process currently owns an empty drop list.
        context.set_return_u32(0);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

impl HostCallHandler for DragFinish {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _drop = context.argument_u32(0)?;
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for DragAcceptFiles {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _window = context.argument_u32(0)?;
        let _accept = context.argument_u32(1)?;
        context.set_stdcall_cleanup(8);
        Ok(())
    }
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
