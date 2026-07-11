//! Target-driven `winmm.dll` multimedia compatibility surface.

use tracing::debug;
use vnrt_win32::{
    ApiKey, ApiRegistry, GuestAddress, HostCallContext, HostCallHandler, Win32Error, read_ansi_z,
};

const MODULE: &str = "winmm.dll";

/// Register the multimedia APIs currently required by the selected Guest.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "mciSendStringA"), MciSendStringA);
    registry.register(ApiKey::new(MODULE, "timeGetTime"), TimeGetTime);
    registry.register(ApiKey::new(MODULE, "timeBeginPeriod"), TimerPeriod);
    registry.register(ApiKey::new(MODULE, "timeEndPeriod"), TimerPeriod);
}

#[derive(Debug, Clone, Copy)]
struct MciSendStringA;

impl HostCallHandler for MciSendStringA {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let command = read_ansi_z(context, GuestAddress(context.argument_u32(0)?))?;
        debug!(command, "guest MCI command");
        let output = GuestAddress(context.argument_u32(1)?);
        let capacity = context.argument_u32(2)?;
        let _callback_window = context.argument_u32(3)?;
        if output.0 != 0 && capacity != 0 {
            context.write_memory(output, &[0])?;
        }
        context.set_return_u32(0); // MCIERR_NO_ERROR
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct TimeGetTime;

impl HostCallHandler for TimeGetTime {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let ticks = context.tick_count();
        context.set_return_u32(ticks);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct TimerPeriod;

impl HostCallHandler for TimerPeriod {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _period = context.argument_u32(0)?;
        context.set_return_u32(0); // TIMERR_NOERROR
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_mci_and_timer_surface() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 4);
    }
}
