//! Initial `user32.dll` API surface and message-queue types.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use tracing::debug;
use vnrt_win32::{
    ApiKey, ApiRegistry, GuestAddress, HostCallContext, HostCallHandler, UnsupportedApi,
    Win32Error, encode_ansi_z, encode_utf16_z, read_ansi_z, read_utf16_z,
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
        CreateWindowEx { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "CreateWindowExW"),
        CreateWindowEx { wide: true },
    );
    registry.register(ApiKey::new(MODULE, "GetMessageA"), GetMessage);
    registry.register(ApiKey::new(MODULE, "GetMessageW"), GetMessage);
    registry.register(ApiKey::new(MODULE, "PeekMessageA"), PeekMessage);
    registry.register(ApiKey::new(MODULE, "PeekMessageW"), PeekMessage);
    registry.register(ApiKey::new(MODULE, "PostMessageA"), PostMessage);
    registry.register(ApiKey::new(MODULE, "PostMessageW"), PostMessage);
    registry.register(ApiKey::new(MODULE, "PostQuitMessage"), PostQuitMessage);
    registry.register(ApiKey::new(MODULE, "TranslateMessage"), TranslateMessage);
    registry.register(ApiKey::new(MODULE, "DispatchMessageA"), DispatchMessage);
    registry.register(ApiKey::new(MODULE, "DispatchMessageW"), DispatchMessage);
    registry.register(
        ApiKey::new(MODULE, "DefWindowProcA"),
        DefWindowProc { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "DefWindowProcW"),
        DefWindowProc { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "ChangeDisplaySettingsExA"),
        ChangeDisplaySettings {
            extended: true,
            wide: false,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "ChangeDisplaySettingsExW"),
        ChangeDisplaySettings {
            extended: true,
            wide: true,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "ChangeDisplaySettingsA"),
        ChangeDisplaySettings {
            extended: false,
            wide: false,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "ChangeDisplaySettingsW"),
        ChangeDisplaySettings {
            extended: false,
            wide: true,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "EnumDisplaySettingsA"),
        EnumDisplaySettings { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "EnumDisplaySettingsW"),
        EnumDisplaySettings { wide: true },
    );
    registry.register(ApiKey::new(MODULE, "GetDC"), GetDc);
    registry.register(ApiKey::new(MODULE, "ReleaseDC"), ReleaseDc);
    registry.register(ApiKey::new(MODULE, "GetSystemMetrics"), GetSystemMetrics);
    registry.register(ApiKey::new(MODULE, "MonitorFromWindow"), MonitorFromWindow);
    registry.register(ApiKey::new(MODULE, "MonitorFromRect"), MonitorFromRect);
    registry.register(ApiKey::new(MODULE, "MonitorFromPoint"), MonitorFromPoint);
    registry.register(ApiKey::new(MODULE, "GetMonitorInfoA"), GetMonitorInfoA);
    registry.register(ApiKey::new(MODULE, "ShowCursor"), ShowCursor);
    registry.register(ApiKey::new(MODULE, "SetCursor"), SetCursor);
    registry.register(ApiKey::new(MODULE, "SetCursorPos"), SetCursorPos);
    registry.register(ApiKey::new(MODULE, "GetCursorPos"), GetCursorPos);
    registry.register(ApiKey::new(MODULE, "UpdateWindow"), UpdateWindow);
    registry.register(ApiKey::new(MODULE, "InvalidateRect"), InvalidateRect);
    registry.register(ApiKey::new(MODULE, "SetWindowLongA"), SetWindowLong);
    registry.register(ApiKey::new(MODULE, "SetWindowLongW"), SetWindowLong);
    registry.register(ApiKey::new(MODULE, "GetWindowLongA"), GetWindowLong);
    registry.register(ApiKey::new(MODULE, "GetWindowLongW"), GetWindowLong);
    registry.register(ApiKey::new(MODULE, "LoadCursorA"), LoadCursor);
    registry.register(ApiKey::new(MODULE, "LoadCursorW"), LoadCursor);
    registry.register(ApiKey::new(MODULE, "LoadIconA"), LoadIcon);
    registry.register(ApiKey::new(MODULE, "LoadIconW"), LoadIcon);
    registry.register(ApiKey::new(MODULE, "DestroyIcon"), DestroyIcon);
    registry.register(
        ApiKey::new(MODULE, "CreateIconIndirect"),
        CreateIconIndirect,
    );
    registry.register(ApiKey::new(MODULE, "OpenIcon"), OpenIcon);
    registry.register(
        ApiKey::new(MODULE, "RegisterClassExA"),
        RegisterClass { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "RegisterClassExW"),
        RegisterClass { wide: true },
    );
    registry.register(
        ApiKey::new(MODULE, "RegisterClassA"),
        RegisterClass { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "RegisterClassW"),
        RegisterClass { wide: true },
    );
    registry.register(ApiKey::new(MODULE, "SetWindowRgn"), SetWindowRgn);
    registry.register(
        ApiKey::new(MODULE, "GetClassNameA"),
        GetClassName { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetClassNameW"),
        GetClassName { wide: true },
    );
    registry.register(ApiKey::new(MODULE, "SetClassLongA"), SetClassLong);
    registry.register(ApiKey::new(MODULE, "SetClassLongW"), SetClassLong);
    registry.register(ApiKey::new(MODULE, "GetClassLongA"), GetClassLong);
    registry.register(ApiKey::new(MODULE, "GetClassLongW"), GetClassLong);
    registry.register(ApiKey::new(MODULE, "ClientToScreen"), ConvertWindowPoint);
    registry.register(ApiKey::new(MODULE, "ScreenToClient"), ConvertWindowPoint);
    registry.register(ApiKey::new(MODULE, "GetSystemMenu"), GetSystemMenu);
    registry.register(ApiKey::new(MODULE, "DeleteMenu"), DeleteMenu);
    registry.register(ApiKey::new(MODULE, "CreateMenu"), CreateMenu);
    registry.register(ApiKey::new(MODULE, "CreatePopupMenu"), CreateMenu);
    registry.register(ApiKey::new(MODULE, "DestroyMenu"), DestroyMenu);
    registry.register(ApiKey::new(MODULE, "IsMenu"), IsMenu);
    registry.register(ApiKey::new(MODULE, "EnumWindows"), EnumWindows);
    registry.register(
        ApiKey::new(MODULE, "SystemParametersInfoA"),
        SystemParametersInfo,
    );
    registry.register(
        ApiKey::new(MODULE, "SystemParametersInfoW"),
        SystemParametersInfo,
    );
    registry.register(ApiKey::new(MODULE, "InsertMenuItemA"), InsertMenuItem);
    registry.register(ApiKey::new(MODULE, "InsertMenuItemW"), InsertMenuItem);
    registry.register(ApiKey::new(MODULE, "DrawMenuBar"), DrawMenuBar);
    registry.register(ApiKey::new(MODULE, "SetMenu"), SetMenu);
    registry.register(ApiKey::new(MODULE, "GetMenu"), GetMenu);
    registry.register(
        ApiKey::new(MODULE, "GetWindowRect"),
        GetWindowRectangle { client: false },
    );
    registry.register(
        ApiKey::new(MODULE, "GetClientRect"),
        GetWindowRectangle { client: true },
    );
    registry.register(ApiKey::new(MODULE, "OpenClipboard"), OpenClipboard);
    registry.register(ApiKey::new(MODULE, "CloseClipboard"), CloseClipboard);
    registry.register(ApiKey::new(MODULE, "EmptyClipboard"), EmptyClipboard);
    registry.register(ApiKey::new(MODULE, "GetClipboardData"), GetClipboardData);
    registry.register(ApiKey::new(MODULE, "SetClipboardData"), SetClipboardData);
    registry.register(
        ApiKey::new(MODULE, "IsClipboardFormatAvailable"),
        IsClipboardFormatAvailable,
    );
    registry.register(ApiKey::new(MODULE, "GetDlgItem"), GetDlgItem);
    registry.register(ApiKey::new(MODULE, "SetWindowTextA"), SetWindowTextA);
    registry.register(ApiKey::new(MODULE, "GetWindowTextA"), GetWindowTextA);
    registry.register(ApiKey::new(MODULE, "SendMessageA"), SendMessageA);
    registry.register(
        ApiKey::new(MODULE, "SendMessageTimeoutA"),
        SendMessageTimeout,
    );
    registry.register(
        ApiKey::new(MODULE, "SendMessageTimeoutW"),
        SendMessageTimeout,
    );
    registry.register(ApiKey::new(MODULE, "IsWindow"), IsWindow);
    registry.register(ApiKey::new(MODULE, "IsWindowVisible"), IsWindowVisible);
    registry.register(ApiKey::new(MODULE, "ShowWindow"), ShowWindow);
    registry.register(ApiKey::new(MODULE, "EnableWindow"), EnableWindow);
    registry.register(ApiKey::new(MODULE, "IsWindowEnabled"), IsWindowEnabled);
    registry.register(ApiKey::new(MODULE, "MoveWindow"), MoveWindow);
    registry.register(ApiKey::new(MODULE, "SetWindowPos"), SetWindowPos);
    registry.register(ApiKey::new(MODULE, "SetRect"), SetRect);
    registry.register(
        ApiKey::new(MODULE, "AdjustWindowRect"),
        AdjustWindowRect { extended: false },
    );
    registry.register(
        ApiKey::new(MODULE, "AdjustWindowRectEx"),
        AdjustWindowRect { extended: true },
    );
    registry.register(
        ApiKey::new(MODULE, "SetWindowPlacement"),
        WindowPlacement { set: true },
    );
    registry.register(
        ApiKey::new(MODULE, "GetWindowPlacement"),
        WindowPlacement { set: false },
    );
    registry.register(ApiKey::new(MODULE, "SetFocus"), SetFocus);
    registry.register(ApiKey::new(MODULE, "DialogBoxParamA"), DialogBoxParamA);
    registry.register(ApiKey::new(MODULE, "EndDialog"), EndDialog);
    registry.register(ApiKey::new(MODULE, "DestroyWindow"), DestroyWindow);
}

#[derive(Debug, Clone, Copy)]
struct GetDc;

#[derive(Debug, Clone, Copy)]
struct GetMessage;

#[derive(Debug, Clone, Copy)]
struct PeekMessage;

#[derive(Debug, Clone, Copy)]
struct PostMessage;

#[derive(Debug, Clone, Copy)]
struct PostQuitMessage;

#[derive(Debug, Clone, Copy)]
struct TranslateMessage;

#[derive(Debug, Clone, Copy)]
struct DispatchMessage;

#[derive(Debug, Clone, Copy)]
struct DefWindowProc {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct ChangeDisplaySettings {
    extended: bool,
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct EnumDisplaySettings {
    wide: bool,
}

impl HostCallHandler for EnumDisplaySettings {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        const DM_BITSPERPEL: u32 = 0x0004_0000;
        const DM_PELSWIDTH: u32 = 0x0008_0000;
        const DM_PELSHEIGHT: u32 = 0x0010_0000;
        const DM_DISPLAYFREQUENCY: u32 = 0x0040_0000;
        let _device_name = context.argument_u32(0)?;
        let mode_number = context.argument_u32(1)?;
        let output = GuestAddress(context.argument_u32(2)?);
        if !matches!(mode_number, 0 | 0xffff_fffe | 0xffff_ffff) {
            context.set_return_u32(0);
            context.set_stdcall_cleanup(12);
            return Ok(());
        }
        let (
            size,
            size_offset,
            fields_offset,
            bits_offset,
            width_offset,
            height_offset,
            frequency_offset,
        ) = if self.wide {
            (220, 68, 72, 168, 172, 176, 184)
        } else {
            (124, 36, 40, 104, 108, 112, 120)
        };
        let mut mode = vec![0; size];
        if self.wide {
            let name = encode_utf16_z(r"\\.\DISPLAY1");
            mode[..name.len().min(64)].copy_from_slice(&name[..name.len().min(64)]);
        } else {
            let name = encode_ansi_z(r"\\.\DISPLAY1");
            mode[..name.len().min(32)].copy_from_slice(&name[..name.len().min(32)]);
        }
        mode[size_offset..size_offset + 2].copy_from_slice(&(size as u16).to_le_bytes());
        let fields = DM_BITSPERPEL | DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
        mode[fields_offset..fields_offset + 4].copy_from_slice(&fields.to_le_bytes());
        mode[bits_offset..bits_offset + 4].copy_from_slice(&32_u32.to_le_bytes());
        let (width, height) = context.primary_display_size();
        mode[width_offset..width_offset + 4].copy_from_slice(&width.to_le_bytes());
        mode[height_offset..height_offset + 4].copy_from_slice(&height.to_le_bytes());
        mode[frequency_offset..frequency_offset + 4].copy_from_slice(&60_u32.to_le_bytes());
        context.write_memory(output, &mode)?;
        context.set_return_u32(1);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for ChangeDisplaySettings {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        const DM_PELSWIDTH: u32 = 0x0008_0000;
        const DM_PELSHEIGHT: u32 = 0x0010_0000;
        let (mode_argument, flags_index, cleanup) = if self.extended {
            let _device_name = context.argument_u32(0)?;
            (context.argument_u32(1)?, 2, 20)
        } else {
            (context.argument_u32(0)?, 1, 8)
        };
        let _flags = context.argument_u32(flags_index)?;
        if mode_argument == 0 {
            context.set_primary_display_size(1280, 720);
        } else {
            let (mode_size, fields_offset, width_offset, height_offset) = if self.wide {
                (220, 72, 172, 176)
            } else {
                (124, 40, 108, 112)
            };
            let mut mode = vec![0; mode_size];
            context.read_memory(GuestAddress(mode_argument), &mut mode)?;
            let fields = u32::from_le_bytes(
                mode[fields_offset..fields_offset + 4]
                    .try_into()
                    .expect("dmFields"),
            );
            let (current_width, current_height) = context.primary_display_size();
            let width = if fields & DM_PELSWIDTH != 0 {
                u32::from_le_bytes(
                    mode[width_offset..width_offset + 4]
                        .try_into()
                        .expect("dmPelsWidth"),
                )
            } else {
                current_width
            };
            let height = if fields & DM_PELSHEIGHT != 0 {
                u32::from_le_bytes(
                    mode[height_offset..height_offset + 4]
                        .try_into()
                        .expect("dmPelsHeight"),
                )
            } else {
                current_height
            };
            if width != 0 && height != 0 {
                context.set_primary_display_size(width, height);
            }
        }
        context.set_return_u32(0); // DISP_CHANGE_SUCCESSFUL
        context.set_stdcall_cleanup(cleanup);
        Ok(())
    }
}

impl HostCallHandler for DefWindowProc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        const WM_CREATE: u32 = 0x0001;
        const WM_SETTEXT: u32 = 0x000c;
        const WM_GETTEXT: u32 = 0x000d;
        const WM_GETTEXTLENGTH: u32 = 0x000e;
        const WM_CLOSE: u32 = 0x0010;
        const WM_ERASEBKGND: u32 = 0x0014;
        const WM_NCCREATE: u32 = 0x0081;
        let window = context.argument_u32(0)?;
        let message = context.argument_u32(1)?;
        let wparam = context.argument_u32(2)?;
        let lparam = GuestAddress(context.argument_u32(3)?);
        let result = match message {
            WM_NCCREATE => 1,
            WM_CREATE => 0,
            WM_SETTEXT => {
                let title = if lparam.0 == 0 {
                    String::new()
                } else if self.wide {
                    read_utf16_z(context, lparam)?
                } else {
                    read_ansi_z(context, lparam)?
                };
                u32::from(context.set_window_title(window, &title))
            }
            WM_GETTEXT => write_window_title(context, window, lparam, wparam as usize, self.wide)?,
            WM_GETTEXTLENGTH => context.window_title(window).map_or(0, |title| {
                if self.wide {
                    title.encode_utf16().count() as u32
                } else {
                    encode_ansi_z(&title).len().saturating_sub(1) as u32
                }
            }),
            WM_CLOSE => {
                context.remove_window(window);
                0
            }
            WM_ERASEBKGND => 1,
            _ => 0,
        };
        context.set_return_u32(result);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

fn write_window_title(
    context: &mut dyn HostCallContext,
    window: u32,
    output: GuestAddress,
    capacity: usize,
    wide: bool,
) -> Result<u32, Win32Error> {
    let Some(title) = context.window_title(window) else {
        return Ok(0);
    };
    let encoded = if wide {
        encode_utf16_z(&title)
    } else {
        encode_ansi_z(&title)
    };
    let unit_size = if wide { 2 } else { 1 };
    let copied_units = (encoded.len() / unit_size)
        .saturating_sub(1)
        .min(capacity.saturating_sub(1));
    if capacity != 0 {
        let copied_bytes = copied_units * unit_size;
        let mut bytes = encoded[..copied_bytes].to_vec();
        bytes.resize(copied_bytes + unit_size, 0);
        context.write_memory(output, &bytes)?;
    }
    Ok(copied_units as u32)
}

fn write_message(
    context: &mut dyn HostCallContext,
    output: GuestAddress,
    message: (u32, u32, u32, u32),
) -> Result<(), Win32Error> {
    let (window, identifier, wparam, lparam) = message;
    let fields = [
        window,
        identifier,
        wparam,
        lparam,
        context.tick_count(),
        0,
        0,
    ];
    let bytes = fields
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    context.write_memory(output, &bytes)
}

impl HostCallHandler for GetMessage {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let output = GuestAddress(context.argument_u32(0)?);
        let _window_filter = context.argument_u32(1)?;
        let minimum = context.argument_u32(2)?;
        let maximum = context.argument_u32(3)?;
        if let Some(message) = context.next_thread_message(true, minimum, maximum) {
            write_message(context, output, message)?;
            context.set_last_error(0);
            context.set_return_u32(u32::from(message.1 != 0x0012));
        } else {
            // A native backend will block and pump platform events here. Until
            // then, report the absent scheduler instead of fabricating WM_QUIT.
            context.set_last_error(50); // ERROR_NOT_SUPPORTED
            context.set_return_u32(u32::MAX);
        }
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

impl HostCallHandler for PeekMessage {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let output = GuestAddress(context.argument_u32(0)?);
        let _window_filter = context.argument_u32(1)?;
        let minimum = context.argument_u32(2)?;
        let maximum = context.argument_u32(3)?;
        let remove = context.argument_u32(4)? & 1 != 0;
        if let Some(message) = context.next_thread_message(remove, minimum, maximum) {
            write_message(context, output, message)?;
            context.set_return_u32(1);
        } else {
            context.set_return_u32(0);
        }
        context.set_stdcall_cleanup(20);
        Ok(())
    }
}

impl HostCallHandler for PostMessage {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let message = context.argument_u32(1)?;
        let wparam = context.argument_u32(2)?;
        let lparam = context.argument_u32(3)?;
        if window == 0 || window == 0xffff || context.is_window(window) {
            context.post_thread_message(window, message, wparam, lparam);
            context.set_return_u32(1);
        } else {
            context.set_last_error(1400); // ERROR_INVALID_WINDOW_HANDLE
            context.set_return_u32(0);
        }
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

impl HostCallHandler for PostQuitMessage {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let exit_code = context.argument_u32(0)?;
        context.post_thread_message(0, 0x0012, exit_code, 0); // WM_QUIT
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for TranslateMessage {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let message = GuestAddress(context.argument_u32(0)?);
        let mut bytes = [0; 28];
        context.read_memory(message, &mut bytes)?;
        context.set_return_u32(1);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for DispatchMessage {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let message = GuestAddress(context.argument_u32(0)?);
        let mut bytes = [0; 28];
        context.read_memory(message, &mut bytes)?;
        let field = |offset: usize| {
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("MSG field"))
        };
        let window = field(0);
        context.set_return_u32(0);
        context.set_stdcall_cleanup(4);
        if let Some(callback) = context.guest_callback_target(window) {
            context.request_guest_callback(callback, &[window, field(4), field(8), field(12)])?;
            context.use_guest_callback_return_value();
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SetCursor;

#[derive(Debug, Clone, Copy)]
struct SetCursorPos;

#[derive(Debug, Clone, Copy)]
struct GetCursorPos;

#[derive(Debug, Clone, Copy)]
struct UpdateWindow;

#[derive(Debug, Clone, Copy)]
struct InvalidateRect;

#[derive(Debug, Clone, Copy)]
struct SetWindowLong;

#[derive(Debug, Clone, Copy)]
struct GetWindowLong;

impl HostCallHandler for SetWindowLong {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let index = context.argument_u32(1)? as i32;
        let value = context.argument_u32(2)?;
        if let Some(previous) = context.replace_window_long(window, index, value) {
            context.set_last_error(0);
            context.set_return_u32(previous);
        } else {
            context.set_last_error(1400); // ERROR_INVALID_WINDOW_HANDLE
            context.set_return_u32(0);
        }
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for GetWindowLong {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let index = context.argument_u32(1)? as i32;
        if let Some(value) = context.window_long(window, index) {
            context.set_last_error(0);
            context.set_return_u32(value);
        } else {
            context.set_last_error(1400); // ERROR_INVALID_WINDOW_HANDLE
            context.set_return_u32(0);
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for UpdateWindow {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        const WM_PAINT: u32 = 0x000f;
        let window = context.argument_u32(0)?;
        if !context.is_window(window) {
            context.set_last_error(1400); // ERROR_INVALID_WINDOW_HANDLE
            context.set_return_u32(0);
        } else {
            context.set_return_u32(1);
            if let Some(callback) = context.guest_callback_target(window) {
                context.request_guest_callback(callback, &[window, WM_PAINT, 0, 0])?;
            }
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for InvalidateRect {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let rectangle = GuestAddress(context.argument_u32(1)?);
        let _erase = context.argument_u32(2)?;
        if rectangle.0 != 0 {
            let mut bytes = [0; 16];
            context.read_memory(rectangle, &mut bytes)?;
        }
        context.set_return_u32(u32::from(context.is_window(window)));
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for SetCursorPos {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let x = context.argument_u32(0)? as i32;
        let y = context.argument_u32(1)? as i32;
        context.set_cursor_position(x, y);
        context.set_return_u32(1);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for GetCursorPos {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let output = GuestAddress(context.argument_u32(0)?);
        let (x, y) = context.cursor_position();
        let bytes = [x, y]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>();
        context.write_memory(output, &bytes)?;
        context.set_return_u32(1);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct LoadCursor;

#[derive(Debug, Clone, Copy)]
struct LoadIcon;

#[derive(Debug, Clone, Copy)]
struct DestroyIcon;

#[derive(Debug, Clone, Copy)]
struct CreateIconIndirect;

#[derive(Debug, Clone, Copy)]
struct OpenIcon;

#[derive(Debug, Clone, Copy)]
struct RegisterClass {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct CreateWindowEx {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct GetClassName {
    wide: bool,
}

impl HostCallHandler for GetClassName {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let output = GuestAddress(context.argument_u32(1)?);
        let capacity = context.argument_u32(2)? as usize;
        let name = if window == STARTUP_DIALOG_HANDLE {
            Some("#32770".to_owned())
        } else {
            context.window_class_name(window)
        };
        let Some(name) = name else {
            context.set_last_error(1400); // ERROR_INVALID_WINDOW_HANDLE
            context.set_return_u32(0);
            context.set_stdcall_cleanup(12);
            return Ok(());
        };
        let encoded = if self.wide {
            encode_utf16_z(&name)
        } else {
            encode_ansi_z(&name)
        };
        let unit_size = if self.wide { 2 } else { 1 };
        let available_units = encoded.len() / unit_size;
        let copied_units = available_units
            .saturating_sub(1)
            .min(capacity.saturating_sub(1));
        let copied_bytes = copied_units * unit_size;
        if capacity != 0 {
            let mut bytes = encoded[..copied_bytes].to_vec();
            bytes.resize(copied_bytes + unit_size, 0);
            context.write_memory(output, &bytes)?;
        }
        context.set_last_error(0);
        context.set_return_u32(copied_units as u32);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for CreateWindowEx {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        const WM_NCCREATE: u32 = 0x0081;
        const WM_CREATE: u32 = 0x0001;
        let ex_style = context.argument_u32(0)?;
        let class_argument = context.argument_u32(1)?;
        let title_argument = context.argument_u32(2)?;
        let style = context.argument_u32(3)?;
        let x = context.argument_u32(4)?;
        let y = context.argument_u32(5)?;
        let width = context.argument_u32(6)?;
        let height = context.argument_u32(7)?;
        let parent = context.argument_u32(8)?;
        let menu = context.argument_u32(9)?;
        let instance = context.argument_u32(10)?;
        let parameter = context.argument_u32(11)?;

        let (class_name, callback) = if class_argument <= u32::from(u16::MAX) {
            let atom = class_argument as u16;
            (
                context.window_class_name_by_atom(atom),
                context.window_class_callback_by_atom(atom),
            )
        } else {
            let name = if self.wide {
                read_utf16_z(context, GuestAddress(class_argument))?
            } else {
                read_ansi_z(context, GuestAddress(class_argument))?
            };
            let callback = context.window_class_callback_by_name(&name);
            (Some(name), callback)
        };
        let (Some(class_name), Some(callback)) = (class_name, callback) else {
            context.set_last_error(1407); // ERROR_CANNOT_FIND_WND_CLASS
            context.set_return_u32(0);
            context.set_stdcall_cleanup(48);
            return Ok(());
        };

        let title = if title_argument != 0 {
            if self.wide {
                read_utf16_z(context, GuestAddress(title_argument))?
            } else {
                read_ansi_z(context, GuestAddress(title_argument))?
            }
        } else {
            String::new()
        };
        let window = context.create_window(&class_name, &title, style & 0x1000_0000 != 0);
        context.replace_window_long(window, -20, ex_style);
        context.replace_window_long(window, -16, style);
        context.replace_window_long(window, -12, menu);
        context.replace_window_long(window, -8, parent);
        context.replace_window_long(window, -6, instance);
        context.replace_window_long(window, -4, callback.0);
        update_window_geometry(
            context,
            window,
            x as i32,
            y as i32,
            width as i32,
            height as i32,
        );
        context.register_guest_callback_target(window, callback);

        let create_structure = context.allocate_virtual_memory(48, true, true, false)?;
        let fields = [
            parameter,
            instance,
            menu,
            parent,
            height,
            width,
            y,
            x,
            style,
            title_argument,
            class_argument,
            ex_style,
        ];
        let bytes = fields
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        context.write_memory(create_structure, &bytes)?;
        context.request_guest_callback(callback, &[window, WM_NCCREATE, 0, create_structure.0])?;
        context.request_guest_callback(callback, &[window, WM_CREATE, 0, create_structure.0])?;
        context.set_last_error(0);
        context.set_return_u32(window);
        context.set_stdcall_cleanup(48);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SetWindowRgn;

impl HostCallHandler for SetWindowRgn {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let region = context.argument_u32(1)?;
        let _redraw = context.argument_u32(2)?;
        if window == 0 {
            context.set_last_error(1400); // ERROR_INVALID_WINDOW_HANDLE
            context.set_return_u32(0);
        } else {
            context.set_window_region(window, region);
            context.set_last_error(0);
            context.set_return_u32(1);
        }
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for RegisterClass {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let class = GuestAddress(context.argument_u32(0)?);
        let mut prefix = [0; 12];
        context.read_memory(class, &mut prefix)?;
        let first = u32::from_le_bytes(prefix[0..4].try_into().expect("fixed class prefix"));
        let extended = first == 48;
        let callback_offset = if extended { 8 } else { 4 };
        let name_offset = if extended { 40 } else { 36 };
        let structure_size = if extended { 48 } else { 40 };
        let mut bytes = vec![0; structure_size];
        context.read_memory(class, &mut bytes)?;
        let callback = GuestAddress(u32::from_le_bytes(
            bytes[callback_offset..callback_offset + 4]
                .try_into()
                .expect("fixed WNDCLASS callback"),
        ));
        let name_address = GuestAddress(u32::from_le_bytes(
            bytes[name_offset..name_offset + 4]
                .try_into()
                .expect("fixed WNDCLASS name"),
        ));
        if callback.0 == 0 || name_address.0 == 0 {
            context.set_last_error(87); // ERROR_INVALID_PARAMETER
            context.set_return_u32(0);
        } else {
            let name = if self.wide {
                read_utf16_z(context, name_address)?
            } else {
                read_ansi_z(context, name_address)?
            };
            if let Some(atom) = context.register_window_class(&name, callback) {
                context.set_last_error(0);
                context.set_return_u32(u32::from(atom));
            } else {
                context.set_last_error(1410); // ERROR_CLASS_ALREADY_EXISTS
                context.set_return_u32(0);
            }
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for OpenIcon {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        context.set_return_u32(u32::from(window != 0));
        if window == 0 {
            context.set_last_error(1400); // ERROR_INVALID_WINDOW_HANDLE
        } else {
            context.set_last_error(0);
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for CreateIconIndirect {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let info = GuestAddress(context.argument_u32(0)?);
        let mut icon_info = [0; 20];
        context.read_memory(info, &mut icon_info)?;
        let mask = u32::from_le_bytes(icon_info[12..16].try_into().expect("fixed ICONINFO"));
        let color = u32::from_le_bytes(icon_info[16..20].try_into().expect("fixed ICONINFO"));
        if mask == 0 && color == 0 {
            context.set_last_error(87); // ERROR_INVALID_PARAMETER
            context.set_return_u32(0);
        } else {
            context.set_last_error(0);
            let icon = context.create_icon();
            context.set_return_u32(icon);
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for LoadIcon {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        const SYSTEM_ICON_HANDLE_BASE: u32 = 0x0006_0000;
        let instance = context.argument_u32(0)?;
        let resource = context.argument_u32(1)?;
        if instance == 0 && resource <= u32::from(u16::MAX) && resource != 0 {
            context.set_last_error(0);
            context.set_return_u32(SYSTEM_ICON_HANDLE_BASE | resource);
        } else {
            context.set_last_error(1814); // ERROR_RESOURCE_NAME_NOT_FOUND
            context.set_return_u32(0);
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for DestroyIcon {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let icon = context.argument_u32(0)?;
        let is_shared_system_icon = icon & 0xffff_0000 == 0x0006_0000;
        let destroyed = is_shared_system_icon || context.destroy_icon(icon);
        context.set_return_u32(u32::from(destroyed));
        if !destroyed {
            context.set_last_error(6); // ERROR_INVALID_HANDLE
        } else {
            context.set_last_error(0);
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for LoadCursor {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        const SYSTEM_CURSOR_HANDLE_BASE: u32 = 0x0005_0000;
        let instance = context.argument_u32(0)?;
        let resource = context.argument_u32(1)?;
        if instance == 0 && resource <= u32::from(u16::MAX) && resource != 0 {
            context.set_last_error(0);
            context.set_return_u32(SYSTEM_CURSOR_HANDLE_BASE | resource);
        } else {
            // Custom cursor resources require PE group-cursor decoding and a
            // native cursor object. Keep that boundary visible until observed.
            context.set_last_error(1814); // ERROR_RESOURCE_NAME_NOT_FOUND
            context.set_return_u32(0);
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SetClassLong;

#[derive(Debug, Clone, Copy)]
struct GetClassLong;

impl HostCallHandler for SetClassLong {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let index = context.argument_u32(1)? as i32;
        let value = context.argument_u32(2)?;
        if window == 0 {
            context.set_last_error(1400); // ERROR_INVALID_WINDOW_HANDLE
            context.set_return_u32(0);
        } else {
            let previous = context.replace_window_class_long(window, index, value);
            context.set_last_error(0);
            context.set_return_u32(previous);
        }
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for GetClassLong {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let index = context.argument_u32(1)? as i32;
        if window == 0 {
            context.set_last_error(1400); // ERROR_INVALID_WINDOW_HANDLE
            context.set_return_u32(0);
        } else {
            context.set_last_error(0);
            context.set_return_u32(context.window_class_long(window, index));
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct ConvertWindowPoint;

impl HostCallHandler for ConvertWindowPoint {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let point = GuestAddress(context.argument_u32(1)?);
        // Pseudo windows currently sit at the screen origin, making client and
        // screen coordinates identical. Still validate the complete POINT.
        let mut coordinates = [0; 8];
        context.read_memory(point, &mut coordinates)?;
        context.set_return_u32(u32::from(window != 0));
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for SetCursor {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let cursor = context.argument_u32(0)?;
        // Cursor resources are not native objects yet. Returning the modeled
        // cursor keeps the common save/set/restore pattern internally stable.
        context.set_return_u32(cursor);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

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
        let (display_width, display_height) = context.primary_display_size();
        let value = match metric {
            0 | 16 => display_width, // SM_CXSCREEN / SM_CXFULLSCREEN
            1 => display_height,     // SM_CYSCREEN
            17 => 697,               // SM_CYFULLSCREEN
            2 | 3 => 17,             // scroll bars
            4 => 23,                 // SM_CYCAPTION
            5 | 6 => 1,              // border
            7 | 8 => 3,              // dialog frame
            11..=14 => 32,           // icon and cursor dimensions
            15 => 19,                // menu height
            43 => 3,                 // mouse buttons
            49 | 50 => 16,           // small icon dimensions
            67 => 0,                 // normal boot
            80 => 1,                 // one monitor
            0x1000 => 0,             // not a remote session
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

#[derive(Debug, Clone, Copy)]
struct SetMenu;

#[derive(Debug, Clone, Copy)]
struct GetMenu;

#[derive(Debug, Clone, Copy)]
struct GetWindowRectangle {
    client: bool,
}

#[derive(Debug, Clone, Copy)]
struct OpenClipboard;

#[derive(Debug, Clone, Copy)]
struct CloseClipboard;

#[derive(Debug, Clone, Copy)]
struct EmptyClipboard;

#[derive(Debug, Clone, Copy)]
struct GetClipboardData;

#[derive(Debug, Clone, Copy)]
struct SetClipboardData;

#[derive(Debug, Clone, Copy)]
struct IsClipboardFormatAvailable;

impl HostCallHandler for OpenClipboard {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _owner = context.argument_u32(0)?;
        let opened = context.set_clipboard_open(true);
        context.set_return_u32(u32::from(opened));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for CloseClipboard {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let closed = context.set_clipboard_open(false);
        context.set_return_u32(u32::from(closed));
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

impl HostCallHandler for EmptyClipboard {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.clear_clipboard();
        context.set_return_u32(1);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

impl HostCallHandler for GetClipboardData {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let format = context.argument_u32(0)?;
        context.set_return_u32(context.clipboard_data(format).unwrap_or(0));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for SetClipboardData {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let format = context.argument_u32(0)?;
        let handle = context.argument_u32(1)?;
        context.set_clipboard_data(format, handle);
        context.set_return_u32(handle);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for IsClipboardFormatAvailable {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let format = context.argument_u32(0)?;
        context.set_return_u32(u32::from(context.clipboard_data(format).is_some()));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for GetWindowRectangle {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let output = GuestAddress(context.argument_u32(1)?);
        let Some(placement) = context.window_placement(window) else {
            context.set_last_error(1400); // ERROR_INVALID_WINDOW_HANDLE
            context.set_return_u32(0);
            context.set_stdcall_cleanup(8);
            return Ok(());
        };
        let left = read_i32(&placement, 28)?;
        let top = read_i32(&placement, 32)?;
        let right = read_i32(&placement, 36)?;
        let bottom = read_i32(&placement, 40)?;
        let rectangle = if self.client {
            [
                0_i32,
                0,
                right.saturating_sub(left),
                bottom.saturating_sub(top),
            ]
        } else {
            [left, top, right, bottom]
        };
        let bytes = rectangle
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>();
        context.write_memory(output, &bytes)?;
        context.set_last_error(0);
        context.set_return_u32(1);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for SetMenu {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let menu = context.argument_u32(1)?;
        let valid_menu = menu == 0 || menu == SYSTEM_MENU_HANDLE || context.is_menu(menu);
        let success = valid_menu && context.set_window_menu(window, menu);
        context.set_return_u32(u32::from(success));
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for GetMenu {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        context.set_return_u32(context.window_menu(window).unwrap_or(0));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct CreateMenu;

#[derive(Debug, Clone, Copy)]
struct DestroyMenu;

#[derive(Debug, Clone, Copy)]
struct IsMenu;

#[derive(Debug, Clone, Copy)]
struct EnumWindows;

#[derive(Debug, Clone, Copy)]
struct SystemParametersInfo;

impl HostCallHandler for SystemParametersInfo {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let action = context.argument_u32(0)?;
        let _parameter = context.argument_u32(1)?;
        let output = GuestAddress(context.argument_u32(2)?);
        let _flags = context.argument_u32(3)?;
        let value: Option<u32> = match action {
            0x0001 => Some(1),          // SPI_GETBEEP
            0x0005 => Some(1),          // SPI_GETBORDER
            0x000a => Some(31),         // SPI_GETKEYBOARDSPEED
            0x000e => Some(600),        // SPI_GETSCREENSAVETIMEOUT
            0x0010 => Some(0),          // SPI_GETSCREENSAVEACTIVE
            0x0026 => Some(1),          // SPI_GETDRAGFULLWINDOWS
            0x004a => Some(1),          // SPI_GETFONTSMOOTHING
            0x0068 => Some(3),          // SPI_GETWHEELSCROLLLINES
            0x103e | 0x1042 => Some(1), // UI effects / client animation
            _ => None,
        };
        let success = if action == 0x0030 {
            // SPI_GETWORKAREA
            let (width, height) = context.primary_display_size();
            let rectangle = [0_u32, 0, width, height.saturating_sub(23)]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            context.write_memory(output, &rectangle)?;
            true
        } else if action == 0x0003 {
            // SPI_GETMOUSE: two thresholds and acceleration.
            let mouse = [6_u32, 10, 1]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            context.write_memory(output, &mouse)?;
            true
        } else if let Some(value) = value {
            context.write_memory(output, &value.to_le_bytes())?;
            true
        } else {
            false
        };
        context.set_return_u32(u32::from(success));
        context.set_last_error(if success { 0 } else { 87 });
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

impl HostCallHandler for EnumWindows {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let callback = GuestAddress(context.argument_u32(0)?);
        let parameter = context.argument_u32(1)?;
        if callback.0 == 0 {
            context.set_last_error(87); // ERROR_INVALID_PARAMETER
            context.set_return_u32(0);
        } else {
            for window in context.window_handles() {
                context.request_guest_callback(callback, &[window, parameter])?;
            }
            context.set_last_error(0);
            context.set_return_u32(1);
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for CreateMenu {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let menu = context.create_menu();
        context.set_return_u32(menu);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

impl HostCallHandler for DestroyMenu {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let menu = context.argument_u32(0)?;
        let destroyed = context.destroy_menu(menu);
        context.set_return_u32(u32::from(destroyed));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for IsMenu {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let menu = context.argument_u32(0)?;
        context.set_return_u32(u32::from(
            menu == SYSTEM_MENU_HANDLE || context.is_menu(menu),
        ));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct InsertMenuItem;

impl HostCallHandler for InsertMenuItem {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let menu = context.argument_u32(0)?;
        let _item = context.argument_u32(1)?;
        let _by_position = context.argument_u32(2)?;
        let info = GuestAddress(context.argument_u32(3)?);
        let mut header = [0; 8];
        context.read_memory(info, &mut header)?;
        let size = u32::from_le_bytes(header[0..4].try_into().expect("MENUITEMINFO size"));
        let valid =
            (menu == SYSTEM_MENU_HANDLE || context.is_menu(menu)) && matches!(size, 44 | 48);
        context.set_return_u32(u32::from(valid));
        if valid {
            context.set_last_error(0);
        } else {
            context.set_last_error(87); // ERROR_INVALID_PARAMETER
        }
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

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

#[derive(Debug, Clone, Copy)]
struct GetWindowTextA;

impl HostCallHandler for GetWindowTextA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let output = GuestAddress(context.argument_u32(1)?);
        let capacity = context.argument_u32(2)?;
        if window == 0 {
            context.set_last_error(1400); // ERROR_INVALID_WINDOW_HANDLE
            context.set_return_u32(0);
        } else {
            let copied = write_window_title(context, window, output, capacity as usize, false)?;
            context.set_last_error(0);
            context.set_return_u32(copied);
        }
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

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
        let updated = context.set_window_title(window, &value);
        context.set_return_u32(u32::from(updated));
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

#[derive(Debug, Clone, Copy)]
struct SendMessageTimeout;

#[derive(Debug, Clone, Copy)]
struct IsWindow;

#[derive(Debug, Clone, Copy)]
struct IsWindowVisible;

#[derive(Debug, Clone, Copy)]
struct ShowWindow;

#[derive(Debug, Clone, Copy)]
struct EnableWindow;

#[derive(Debug, Clone, Copy)]
struct IsWindowEnabled;

#[derive(Debug, Clone, Copy)]
struct MoveWindow;

#[derive(Debug, Clone, Copy)]
struct SetWindowPos;

#[derive(Debug, Clone, Copy)]
struct SetRect;

#[derive(Debug, Clone, Copy)]
struct AdjustWindowRect {
    extended: bool,
}

impl HostCallHandler for AdjustWindowRect {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        const WS_BORDER: u32 = 0x0080_0000;
        const WS_DLGFRAME: u32 = 0x0040_0000;
        const WS_CAPTION: u32 = WS_BORDER | WS_DLGFRAME;
        const WS_THICKFRAME: u32 = 0x0004_0000;
        const WS_EX_CLIENTEDGE: u32 = 0x0000_0200;
        let rectangle = GuestAddress(context.argument_u32(0)?);
        let style = context.argument_u32(1)?;
        let has_menu = context.argument_u32(2)? != 0;
        let ex_style = if self.extended {
            context.argument_u32(3)?
        } else {
            0
        };
        let mut bytes = [0; 16];
        context.read_memory(rectangle, &mut bytes)?;
        let mut left = i32::from_le_bytes(bytes[0..4].try_into().expect("RECT left"));
        let mut top = i32::from_le_bytes(bytes[4..8].try_into().expect("RECT top"));
        let mut right = i32::from_le_bytes(bytes[8..12].try_into().expect("RECT right"));
        let mut bottom = i32::from_le_bytes(bytes[12..16].try_into().expect("RECT bottom"));
        let mut horizontal = 0_i32;
        let mut vertical = 0_i32;
        if style & WS_THICKFRAME != 0 {
            horizontal += 4;
            vertical += 4;
        } else if style & (WS_BORDER | WS_DLGFRAME) != 0 {
            horizontal += if style & WS_DLGFRAME != 0 { 3 } else { 1 };
            vertical += if style & WS_DLGFRAME != 0 { 3 } else { 1 };
        }
        if ex_style & WS_EX_CLIENTEDGE != 0 {
            horizontal += 2;
            vertical += 2;
        }
        left = left.saturating_sub(horizontal);
        right = right.saturating_add(horizontal);
        top = top.saturating_sub(vertical);
        bottom = bottom.saturating_add(vertical);
        if style & WS_CAPTION == WS_CAPTION {
            top = top.saturating_sub(23);
        }
        if has_menu {
            top = top.saturating_sub(19);
        }
        let adjusted = [left, top, right, bottom]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>();
        context.write_memory(rectangle, &adjusted)?;
        context.set_return_u32(1);
        context.set_stdcall_cleanup(if self.extended { 16 } else { 12 });
        Ok(())
    }
}

impl HostCallHandler for SetRect {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let rectangle = GuestAddress(context.argument_u32(0)?);
        let fields = [
            context.argument_u32(1)?,
            context.argument_u32(2)?,
            context.argument_u32(3)?,
            context.argument_u32(4)?,
        ];
        let bytes = fields
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        context.write_memory(rectangle, &bytes)?;
        context.set_return_u32(1);
        context.set_stdcall_cleanup(20);
        Ok(())
    }
}

impl HostCallHandler for MoveWindow {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let x = context.argument_u32(1)? as i32;
        let y = context.argument_u32(2)? as i32;
        let width = context.argument_u32(3)? as i32;
        let height = context.argument_u32(4)? as i32;
        let _repaint = context.argument_u32(5)?;
        let success = update_window_geometry(context, window, x, y, width, height);
        context.set_return_u32(u32::from(success));
        context.set_stdcall_cleanup(24);
        Ok(())
    }
}

impl HostCallHandler for SetWindowPos {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        const SWP_NOSIZE: u32 = 0x0001;
        const SWP_NOMOVE: u32 = 0x0002;
        const SWP_SHOWWINDOW: u32 = 0x0040;
        const SWP_HIDEWINDOW: u32 = 0x0080;
        let window = context.argument_u32(0)?;
        let _insert_after = context.argument_u32(1)?;
        let x = context.argument_u32(2)? as i32;
        let y = context.argument_u32(3)? as i32;
        let width = context.argument_u32(4)? as i32;
        let height = context.argument_u32(5)? as i32;
        let flags = context.argument_u32(6)?;
        let mut placement = context.window_placement(window);
        let success = if let Some(ref mut placement) = placement {
            let current_left = read_i32(placement, 28)?;
            let current_top = read_i32(placement, 32)?;
            let current_right = read_i32(placement, 36)?;
            let current_bottom = read_i32(placement, 40)?;
            let new_x = if flags & SWP_NOMOVE != 0 {
                current_left
            } else {
                x
            };
            let new_y = if flags & SWP_NOMOVE != 0 {
                current_top
            } else {
                y
            };
            let new_width = if flags & SWP_NOSIZE != 0 {
                current_right.saturating_sub(current_left)
            } else {
                width
            };
            let new_height = if flags & SWP_NOSIZE != 0 {
                current_bottom.saturating_sub(current_top)
            } else {
                height
            };
            write_i32(placement, 28, new_x)?;
            write_i32(placement, 32, new_y)?;
            write_i32(placement, 36, new_x.saturating_add(new_width))?;
            write_i32(placement, 40, new_y.saturating_add(new_height))?;
            context.set_window_placement(window, placement)
        } else {
            false
        };
        if success && flags & SWP_SHOWWINDOW != 0 {
            context.set_window_visible(window, true);
        } else if success && flags & SWP_HIDEWINDOW != 0 {
            context.set_window_visible(window, false);
        }
        context.set_return_u32(u32::from(success));
        context.set_stdcall_cleanup(28);
        Ok(())
    }
}

fn update_window_geometry(
    context: &mut dyn HostCallContext,
    window: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> bool {
    let Some(mut placement) = context.window_placement(window) else {
        return false;
    };
    placement[28..32].copy_from_slice(&x.to_le_bytes());
    placement[32..36].copy_from_slice(&y.to_le_bytes());
    placement[36..40].copy_from_slice(&x.saturating_add(width).to_le_bytes());
    placement[40..44].copy_from_slice(&y.saturating_add(height).to_le_bytes());
    context.set_window_placement(window, &placement)
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32, Win32Error> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(Win32Error::InvalidArgument("window placement field"))?;
    Ok(i32::from_le_bytes(
        value.try_into().expect("four-byte field"),
    ))
}

fn write_i32(bytes: &mut [u8], offset: usize, value: i32) -> Result<(), Win32Error> {
    let target = bytes
        .get_mut(offset..offset + 4)
        .ok_or(Win32Error::InvalidArgument("window placement field"))?;
    target.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

impl HostCallHandler for EnableWindow {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let enabled = context.argument_u32(1)? != 0;
        let previous = context.set_window_enabled(window, enabled);
        context.set_return_u32(u32::from(previous));
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for IsWindowEnabled {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        context.set_return_u32(u32::from(context.is_window_enabled(window)));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct WindowPlacement {
    set: bool,
}

impl HostCallHandler for WindowPlacement {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let placement_address = GuestAddress(context.argument_u32(1)?);
        let success = if self.set {
            let mut placement = [0; 44];
            context.read_memory(placement_address, &mut placement)?;
            let valid_size = u32::from_le_bytes(placement[0..4].try_into().expect("fixed size"))
                == placement.len() as u32;
            if valid_size {
                let show_command =
                    u32::from_le_bytes(placement[8..12].try_into().expect("fixed show command"));
                context.set_window_visible(window, show_command != 0);
                context.set_window_placement(window, &placement)
            } else {
                false
            }
        } else if let Some(placement) = context.window_placement(window) {
            context.write_memory(placement_address, &placement)?;
            true
        } else {
            false
        };
        context.set_return_u32(u32::from(success));
        if success {
            context.set_last_error(0);
        } else {
            context.set_last_error(87); // ERROR_INVALID_PARAMETER
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for IsWindow {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.set_return_u32(u32::from(context.is_window(context.argument_u32(0)?)));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for IsWindowVisible {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.set_return_u32(u32::from(
            context.is_window_visible(context.argument_u32(0)?),
        ));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for ShowWindow {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let command = context.argument_u32(1)?;
        if !context.is_window(window) {
            context.set_last_error(1400); // ERROR_INVALID_WINDOW_HANDLE
            context.set_return_u32(0);
        } else {
            let previous = context.set_window_visible(window, command != 0);
            context.set_last_error(0);
            context.set_return_u32(u32::from(previous));
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for SendMessageTimeout {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let window = context.argument_u32(0)?;
        let message = context.argument_u32(1)?;
        let wparam = context.argument_u32(2)?;
        let lparam = context.argument_u32(3)?;
        let _flags = context.argument_u32(4)?;
        let _timeout = context.argument_u32(5)?;
        let result = GuestAddress(context.argument_u32(6)?);
        if result.0 != 0 {
            context.write_memory(result, &0_u32.to_le_bytes())?;
        }
        context.set_return_u32(1);
        context.set_stdcall_cleanup(28);
        if let Some(callback) = context.guest_callback_target(window) {
            context.request_guest_callback(callback, &[window, message, wparam, lparam])?;
        }
        Ok(())
    }
}

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
        let removed = context.remove_window(window);
        context.set_return_u32(u32::from(removed));
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
