//! Target-driven `gdi32.dll` display and bitmap compatibility surface.

use vnrt_user32::SCREEN_DC_HANDLE;
use vnrt_win32::{ApiKey, ApiRegistry, HostCallContext, HostCallHandler, Win32Error};

const MODULE: &str = "gdi32.dll";

/// Register the GDI APIs currently required by the selected Guest.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "GetDeviceCaps"), GetDeviceCaps);
}

#[derive(Debug, Clone, Copy)]
struct GetDeviceCaps;

impl HostCallHandler for GetDeviceCaps {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        if context.argument_u32(0)? != SCREEN_DC_HANDLE {
            context.set_return_u32(0);
            context.set_stdcall_cleanup(8);
            return Ok(());
        }
        let value = match context.argument_u32(1)? {
            0 => 0x0400,     // DRIVERVERSION
            2 => 1,          // TECHNOLOGY = DT_RASDISPLAY
            4 => 338,        // HORZSIZE, millimeters
            6 => 190,        // VERTSIZE
            8 | 118 => 1280, // HORZRES / DESKTOPHORZRES
            10 | 117 => 720, // VERTRES / DESKTOPVERTRES
            12 => 32,        // BITSPIXEL
            14 => 1,         // PLANES
            24 => u32::MAX,  // NUMCOLORS for a true-color display
            88 | 90 => 96,   // LOGPIXELSX / LOGPIXELSY
            108 => 24,       // COLORRES
            110 => 1280,     // PHYSICALWIDTH
            111 => 720,      // PHYSICALHEIGHT
            112 | 113 => 0,  // PHYSICALOFFSETX / PHYSICALOFFSETY
            116 => 60,       // VREFRESH
            _ => 0,
        };
        context.set_return_u32(value);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_display_capability_query() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 1);
    }
}
