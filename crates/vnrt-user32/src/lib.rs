//! Initial `user32.dll` API surface and message-queue types.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use tracing::debug;
use vnrt_win32::{
    ApiKey, ApiRegistry, GuestAddress, HostCallContext, HostCallHandler, UnsupportedApi,
    Win32Error, read_ansi_z,
};

const MODULE: &str = "user32.dll";
/// Stable pseudo HDC representing the primary display in headless mode.
pub const SCREEN_DC_HANDLE: u32 = 0x0003_0000;
const PRIMARY_MONITOR_HANDLE: u32 = 0x0003_0004;
const STARTUP_DIALOG_HANDLE: u32 = 0x0002_0000;
const SYSTEM_MENU_HANDLE: u32 = 0x0002_0004;
const DIALOG_CONTROL_HANDLE_BASE: u32 = 0x0004_0000;
const WM_INITDIALOG: u32 = 0x0110;
const WM_COMMAND: u32 = 0x0111;
const STARTUP_DIALOG_ACCEPT_ID: u32 = 0x040a;

/// Guest-visible message record, analogous to the stable part of Win32 `MSG`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    /// Target window handle value.
    pub hwnd: u32,
    /// Message identifier.
    pub message: u32,
    /// First message parameter.
    pub wparam: u32,
    /// Second message parameter.
    pub lparam: u32,
}

/// FIFO message queue owned by one guest thread.
#[derive(Debug, Default)]
pub struct MessageQueue {
    messages: VecDeque<Message>,
}

impl MessageQueue {
    /// Add a message to the tail.
    pub fn post(&mut self, message: Message) {
        self.messages.push_back(message);
    }

    /// Remove the oldest message.
    pub fn pop(&mut self) -> Option<Message> {
        self.messages.pop_front()
    }
}

/// Window metadata placeholder until the SDL3 backend owns native windows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Window {
    /// Guest window handle.
    pub hwnd: u32,
    /// UTF-8 host-side title.
    pub title: String,
}

/// Register the current `user32.dll` surface.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(
        ApiKey::new(MODULE, "MessageBoxA"),
        UnsupportedApi::new("user32!MessageBoxA"),
    );
    registry.register(
        ApiKey::new(MODULE, "MessageBoxW"),
        UnsupportedApi::new("user32!MessageBoxW"),
    );
    registry.register(
        ApiKey::new(MODULE, "CreateWindowExA"),
        UnsupportedApi::new("user32!CreateWindowExA"),
    );
    registry.register(
        ApiKey::new(MODULE, "CreateWindowExW"),
        UnsupportedApi::new("user32!CreateWindowExW"),
    );
    registry.register(
        ApiKey::new(MODULE, "GetMessageA"),
        UnsupportedApi::new("user32!GetMessageA"),
    );
    registry.register(
        ApiKey::new(MODULE, "GetMessageW"),
        UnsupportedApi::new("user32!GetMessageW"),
    );
    registry.register(ApiKey::new(MODULE, "GetDC"), GetDc);
    registry.register(ApiKey::new(MODULE, "ReleaseDC"), ReleaseDc);
    registry.register(ApiKey::new(MODULE, "GetSystemMetrics"), GetSystemMetrics);
    registry.register(ApiKey::new(MODULE, "MonitorFromWindow"), MonitorFromWindow);
    registry.register(ApiKey::new(MODULE, "MonitorFromRect"), MonitorFromRect);
    registry.register(ApiKey::new(MODULE, "MonitorFromPoint"), MonitorFromPoint);
    registry.register(ApiKey::new(MODULE, "GetMonitorInfoA"), GetMonitorInfoA);
    registry.register(ApiKey::new(MODULE, "ShowCursor"), ShowCursor);
    registry.register(ApiKey::new(MODULE, "GetSystemMenu"), GetSystemMenu);
    registry.register(ApiKey::new(MODULE, "DeleteMenu"), DeleteMenu);
    registry.register(ApiKey::new(MODULE, "DrawMenuBar"), DrawMenuBar);
    registry.register(ApiKey::new(MODULE, "GetDlgItem"), GetDlgItem);
    registry.register(ApiKey::new(MODULE, "SetWindowTextA"), SetWindowTextA);
    registry.register(ApiKey::new(MODULE, "SendMessageA"), SendMessageA);
    registry.register(ApiKey::new(MODULE, "SetFocus"), SetFocus);
    registry.register(ApiKey::new(MODULE, "DialogBoxParamA"), DialogBoxParamA);
    registry.register(ApiKey::new(MODULE, "EndDialog"), EndDialog);
    registry.register(ApiKey::new(MODULE, "DestroyWindow"), DestroyWindow);
}

#[derive(Debug, Clone, Copy)]
struct GetDc;

impl HostCallHandler for GetDc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        if context.argument_u32(0)? != 0 {
            return Err(Win32Error::Unsupported {
                feature: "GetDC for a window",
            });
        }
        context.set_return_u32(SCREEN_DC_HANDLE);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct ReleaseDc;

impl HostCallHandler for ReleaseDc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _window = context.argument_u32(0)?;
        let dc = context.argument_u32(1)?;
        context.set_return_u32(u32::from(dc == SCREEN_DC_HANDLE));
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetSystemMetrics;

impl HostCallHandler for GetSystemMetrics {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let metric = context.argument_u32(0)?;
        let value = match metric {
            0 | 16 => 1280, // SM_CXSCREEN / SM_CXFULLSCREEN
            1 => 720,       // SM_CYSCREEN
            17 => 697,      // SM_CYFULLSCREEN
            2 | 3 => 17,    // scroll bars
            4 => 23,        // SM_CYCAPTION
            5 | 6 => 1,     // border
            7 | 8 => 3,     // dialog frame
            11..=14 => 32,  // icon and cursor dimensions
            15 => 19,       // menu height
            43 => 3,        // mouse buttons
            49 | 50 => 16,  // small icon dimensions
            67 => 0,        // normal boot
            80 => 1,        // one monitor
            0x1000 => 0,    // not a remote session
            _ => 0,
        };
        context.set_return_u32(value);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct MonitorFromWindow;

impl HostCallHandler for MonitorFromWindow {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _window = context.argument_u32(0)?;
        let flags = context.argument_u32(1)?;
        context.set_return_u32(if flags <= 2 {
            PRIMARY_MONITOR_HANDLE
        } else {
            0
        });
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct MonitorFromRect;

impl HostCallHandler for MonitorFromRect {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let rect = context.argument_u32(0)?;
        let flags = context.argument_u32(1)?;
        context.set_return_u32(if rect != 0 && flags <= 2 {
            PRIMARY_MONITOR_HANDLE
        } else {
            0
        });
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct MonitorFromPoint;

impl HostCallHandler for MonitorFromPoint {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _x = context.argument_u32(0)?;
        let _y = context.argument_u32(1)?;
        let flags = context.argument_u32(2)?;
        context.set_return_u32(if flags <= 2 {
            PRIMARY_MONITOR_HANDLE
        } else {
            0
        });
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetMonitorInfoA;

impl HostCallHandler for GetMonitorInfoA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        if context.argument_u32(0)? != PRIMARY_MONITOR_HANDLE {
            context.set_return_u32(0);
            context.set_stdcall_cleanup(8);
            return Ok(());
        }
        let output = vnrt_win32::GuestAddress(context.argument_u32(1)?);
        let mut size_bytes = [0; 4];
        context.read_memory(output, &mut size_bytes)?;
        let size = u32::from_le_bytes(size_bytes);
        if !matches!(size, 40 | 72) {
            context.set_return_u32(0);
            context.set_stdcall_cleanup(8);
            return Ok(());
        }
        let mut info = vec![0; size as usize];
        info[..4].copy_from_slice(&size.to_le_bytes());
        put_i32(&mut info, 12, 1280)?;
        put_i32(&mut info, 16, 720)?;
        put_i32(&mut info, 28, 1280)?;
        put_i32(&mut info, 32, 697)?;
        info[36..40].copy_from_slice(&1_u32.to_le_bytes());
        if size == 72 {
            let name = b"\\\\.\\DISPLAY1\0";
            info[40..40 + name.len()].copy_from_slice(name);
        }
        context.write_memory(output, &info)?;
        context.set_return_u32(1);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct ShowCursor;

impl HostCallHandler for ShowCursor {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let show = context.argument_u32(0)? != 0;
        let count = context.adjust_cursor_display_count(show);
        context.set_return_u32(u32::from_ne_bytes(count.to_ne_bytes()));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetSystemMenu;

impl HostCallHandler for GetSystemMenu {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let revert = context.argument_u32(1)? != 0;
        context.set_return_u32(if window == STARTUP_DIALOG_HANDLE && !revert {
            SYSTEM_MENU_HANDLE
        } else {
            0
        });
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct DeleteMenu;

impl HostCallHandler for DeleteMenu {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let menu = context.argument_u32(0)?;
        let _item = context.argument_u32(1)?;
        let _flags = context.argument_u32(2)?;
        context.set_return_u32(u32::from(menu == SYSTEM_MENU_HANDLE));
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct DrawMenuBar;

impl HostCallHandler for DrawMenuBar {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        context.set_return_u32(u32::from(window == STARTUP_DIALOG_HANDLE));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetDlgItem;

impl HostCallHandler for GetDlgItem {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let dialog = context.argument_u32(0)?;
        let control_id = context.argument_u32(1)?;
        context.set_return_u32(if dialog == STARTUP_DIALOG_HANDLE && control_id != 0 {
            DIALOG_CONTROL_HANDLE_BASE | (control_id & 0xffff)
        } else {
            0
        });
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SetWindowTextA;

impl HostCallHandler for SetWindowTextA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let text = GuestAddress(context.argument_u32(1)?);
        let value = if text.0 == 0 {
            String::new()
        } else {
            read_ansi_z(context, text)?
        };
        debug!(window, text = value, "set Guest window text");
        context.set_return_u32(u32::from(window != 0));
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct DialogBoxParamA;

impl HostCallHandler for DialogBoxParamA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let instance = context.argument_u32(0)?;
        let template = context.argument_u32(1)?;
        let parent = context.argument_u32(2)?;
        let dialog_proc = context.argument_u32(3)?;
        let parameter = context.argument_u32(4)?;
        debug!(
            instance,
            template,
            parent,
            dialog_proc,
            caller = context.argument_u32(6).ok(),
            outer_caller = context.argument_u32(8).ok(),
            "opening Guest dialog"
        );
        if instance == 0 || template == 0 || parent != 0 || dialog_proc == 0 {
            return Err(Win32Error::InvalidArgument(
                "DialogBoxParamA bootstrap arguments",
            ));
        }
        context.set_return_u32(STARTUP_DIALOG_ACCEPT_ID);
        context.set_stdcall_cleanup(20);
        let callback = GuestAddress(dialog_proc);
        context.register_guest_callback_target(STARTUP_DIALOG_HANDLE, callback);
        context.request_guest_callback(
            callback,
            &[STARTUP_DIALOG_HANDLE, WM_INITDIALOG, 0, parameter],
        )?;
        context.request_guest_callback(
            callback,
            &[
                STARTUP_DIALOG_HANDLE,
                WM_COMMAND,
                STARTUP_DIALOG_ACCEPT_ID,
                0,
            ],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SendMessageA;

impl HostCallHandler for SendMessageA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let message = context.argument_u32(1)?;
        let wparam = context.argument_u32(2)?;
        let lparam = context.argument_u32(3)?;
        context.set_return_u32(0);
        context.set_stdcall_cleanup(16);
        if let Some(callback) = context.guest_callback_target(window) {
            context.request_guest_callback(callback, &[window, message, wparam, lparam])?;
            context.use_guest_callback_return_value();
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SetFocus;

impl HostCallHandler for SetFocus {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let previous = context.replace_focus_window(window);
        context.set_return_u32(previous);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct EndDialog;

impl HostCallHandler for EndDialog {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let dialog = context.argument_u32(0)?;
        let result = context.argument_u32(1)?;
        context.set_return_u32(u32::from(dialog == STARTUP_DIALOG_HANDLE));
        context.set_stdcall_cleanup(8);
        if dialog == STARTUP_DIALOG_HANDLE {
            context.complete_suspended_host_call(result)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct DestroyWindow;

impl HostCallHandler for DestroyWindow {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        context.set_return_u32(u32::from(window != 0));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

fn put_i32(bytes: &mut [u8], offset: usize, value: i32) -> Result<(), Win32Error> {
    let end = offset
        .checked_add(4)
        .ok_or(Win32Error::InvalidArgument("monitor field offset"))?;
    let target = bytes
        .get_mut(offset..end)
        .ok_or(Win32Error::InvalidArgument("monitor field bounds"))?;
    target.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_is_fifo() {
        let mut queue = MessageQueue::default();
        let first = Message {
            hwnd: 1,
            message: 2,
            wparam: 3,
            lparam: 4,
        };
        queue.post(first);
        assert_eq!(queue.pop(), Some(first));
        assert_eq!(queue.pop(), None);
    }
}
