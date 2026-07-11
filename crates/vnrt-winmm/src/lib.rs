//! Target-driven `winmm.dll` multimedia compatibility surface.

use tracing::debug;
use vnrt_win32::{
    ApiKey, ApiRegistry, GuestAddress, Handle, HostCallContext, HostCallHandler, Win32Error,
    read_ansi_z, read_utf16_z,
};

const MODULE: &str = "winmm.dll";

/// Register the multimedia APIs currently required by the selected Guest.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "mciSendStringA"), MciSendStringA);
    registry.register(ApiKey::new(MODULE, "mciSendCommandA"), MciSendCommand);
    registry.register(ApiKey::new(MODULE, "mciSendCommandW"), MciSendCommand);
    registry.register(ApiKey::new(MODULE, "timeGetTime"), TimeGetTime);
    registry.register(ApiKey::new(MODULE, "timeGetDevCaps"), TimeGetDevCaps);
    registry.register(ApiKey::new(MODULE, "timeBeginPeriod"), TimerPeriod);
    registry.register(ApiKey::new(MODULE, "timeEndPeriod"), TimerPeriod);
    registry.register(ApiKey::new(MODULE, "joyGetNumDevs"), JoyGetNumDevs);
    registry.register(ApiKey::new(MODULE, "waveOutGetNumDevs"), WaveOutGetNumDevs);
    registry.register(ApiKey::new(MODULE, "waveInGetNumDevs"), WaveOutGetNumDevs);
    for name in [
        "waveInGetDevCapsA",
        "waveInGetDevCapsW",
        "waveOutGetDevCapsA",
        "waveOutGetDevCapsW",
    ] {
        registry.register(
            ApiKey::new(MODULE, name),
            MultimediaUnavailable {
                cleanup: 12,
                result: 2, // MMSYSERR_BADDEVICEID
            },
        );
    }
    registry.register(
        ApiKey::new(MODULE, "joyGetPos"),
        JoyUnavailable { cleanup: 8 },
    );
    registry.register(
        ApiKey::new(MODULE, "joyGetPosEx"),
        JoyUnavailable { cleanup: 8 },
    );
    registry.register(
        ApiKey::new(MODULE, "joyGetDevCapsA"),
        JoyUnavailable { cleanup: 12 },
    );
    registry.register(
        ApiKey::new(MODULE, "joyGetDevCapsW"),
        JoyUnavailable { cleanup: 12 },
    );
    registry.register(
        ApiKey::new(MODULE, "mmioStringToFOURCCA"),
        MmioStringToFourCc,
    );
    registry.register(ApiKey::new(MODULE, "mmioOpenA"), MmioOpen { wide: false });
    registry.register(ApiKey::new(MODULE, "mmioOpenW"), MmioOpen { wide: true });
    registry.register(ApiKey::new(MODULE, "mmioClose"), MmioClose);
    registry.register(ApiKey::new(MODULE, "mmioRead"), MmioRead);
    registry.register(ApiKey::new(MODULE, "mmioSeek"), MmioSeek);
    registry.register(ApiKey::new(MODULE, "mmioDescend"), MmioDescend);
    registry.register(ApiKey::new(MODULE, "mmioAscend"), MmioAscend);
}

#[derive(Debug, Clone, Copy)]
struct MciSendStringA;

#[derive(Debug, Clone, Copy)]
struct MciSendCommand;

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

impl HostCallHandler for MciSendCommand {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let _device_id = context.argument_u32(0)?;
        let message = context.argument_u32(1)?;
        let flags = context.argument_u32(2)?;
        let _parameters = context.argument_u32(3)?;
        debug!(message, flags, "guest MCI command");
        // Match the existing string-command facade until multimedia device
        // objects are backed by a Host audio implementation.
        context.set_return_u32(0); // MCIERR_NO_ERROR
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct TimeGetTime;

#[derive(Debug, Clone, Copy)]
struct TimeGetDevCaps;

impl HostCallHandler for TimeGetTime {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let ticks = context.tick_count();
        context.set_return_u32(ticks);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

impl HostCallHandler for TimeGetDevCaps {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let output = GuestAddress(context.argument_u32(0)?);
        let size = context.argument_u32(1)?;
        if output.0 == 0 || size < 8 {
            context.set_return_u32(97); // TIMERR_STRUCT
        } else {
            let mut capabilities = [0; 8];
            capabilities[..4].copy_from_slice(&1_u32.to_le_bytes());
            capabilities[4..].copy_from_slice(&1_000_000_u32.to_le_bytes());
            context.write_memory(output, &capabilities)?;
            context.set_return_u32(0); // TIMERR_NOERROR
        }
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct TimerPeriod;

#[derive(Debug, Clone, Copy)]
struct JoyGetNumDevs;

#[derive(Debug, Clone, Copy)]
struct WaveOutGetNumDevs;

#[derive(Debug, Clone, Copy)]
struct JoyUnavailable {
    cleanup: u32,
}

#[derive(Debug, Clone, Copy)]
struct MultimediaUnavailable {
    cleanup: u32,
    result: u32,
}

#[derive(Debug, Clone, Copy)]
struct MmioStringToFourCc;

#[derive(Debug, Clone, Copy)]
struct MmioOpen {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct MmioClose;

#[derive(Debug, Clone, Copy)]
struct MmioRead;

#[derive(Debug, Clone, Copy)]
struct MmioSeek;

#[derive(Debug, Clone, Copy)]
struct MmioDescend;

#[derive(Debug, Clone, Copy)]
struct MmioAscend;

const MMIO_WRITE: u32 = 0x0000_0001;
const MMIO_READWRITE: u32 = 0x0000_0002;
const MMIO_CREATE: u32 = 0x0000_1000;
const MMIOERR_CANNOTOPEN: u32 = 257;
const MMIOERR_CANNOTCLOSE: u32 = 260;
const MMIOERR_CANNOTSEEK: u32 = 261;
const MMIOERR_CHUNKNOTFOUND: u32 = 265;
const MMIO_FINDCHUNK: u32 = 0x0010;
const MMIO_FINDRIFF: u32 = 0x0020;
const MMIO_FINDLIST: u32 = 0x0040;
const FOURCC_RIFF: u32 = u32::from_le_bytes(*b"RIFF");
const FOURCC_LIST: u32 = u32::from_le_bytes(*b"LIST");

impl HostCallHandler for MmioStringToFourCc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let value = read_ansi_z(context, GuestAddress(context.argument_u32(0)?))?;
        let flags = context.argument_u32(1)?;
        let mut fourcc = [b' '; 4];
        for (target, source) in fourcc.iter_mut().zip(value.bytes()) {
            *target = if flags & 0x10 != 0 {
                source.to_ascii_uppercase()
            } else {
                source
            };
        }
        context.set_return_u32(u32::from_le_bytes(fourcc));
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for MmioOpen {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let path_address = GuestAddress(context.argument_u32(0)?);
        let info = GuestAddress(context.argument_u32(1)?);
        let flags = context.argument_u32(2)?;
        let path = if self.wide {
            read_utf16_z(context, path_address)?
        } else {
            read_ansi_z(context, path_address)?
        };

        // A caller-supplied IOProc changes HMMIO into a programmable stream.
        // Keep that extension explicit until a target needs it; ordinary files
        // use the same sandboxed handle table as Kernel32 file APIs.
        if info.0 != 0 {
            let mut io_proc = [0; 4];
            context.read_memory(GuestAddress(info.0 + 8), &mut io_proc)?;
            if u32::from_le_bytes(io_proc) != 0 {
                context.set_return_u32(0);
                context.set_stdcall_cleanup(12);
                return Ok(());
            }
        }

        let access = flags & 0x3;
        let opened = match access {
            0 => context.open_file_read(&path),
            MMIO_WRITE | MMIO_READWRITE => {
                let readable = access == MMIO_READWRITE;
                let disposition = if flags & MMIO_CREATE != 0 { 2 } else { 3 };
                context
                    .open_file(&path, readable, true, disposition)
                    .map(|(handle, _)| handle)
            }
            _ => unreachable!("masked MMIO access mode"),
        };

        match opened {
            Ok(handle) => {
                if info.0 != 0 {
                    context.write_memory(GuestAddress(info.0 + 4), &0_u32.to_le_bytes())?;
                    context.write_memory(GuestAddress(info.0 + 12), &0_u32.to_le_bytes())?;
                }
                context.set_return_u32(handle.0);
            }
            Err(_) => {
                if info.0 != 0 {
                    context.write_memory(
                        GuestAddress(info.0 + 12),
                        &MMIOERR_CANNOTOPEN.to_le_bytes(),
                    )?;
                }
                context.set_return_u32(0);
            }
        }
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for MmioClose {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        let _flags = context.argument_u32(1)?;
        let result = if context.close_file(handle).is_ok() {
            0
        } else {
            MMIOERR_CANNOTCLOSE
        };
        context.set_return_u32(result);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for MmioRead {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        let output = GuestAddress(context.argument_u32(1)?);
        let length = usize::try_from(context.argument_u32(2)?)
            .map_err(|_| Win32Error::InvalidArgument("mmioRead length overflow"))?;
        match context.read_file(handle, length) {
            Ok(bytes) => {
                context.write_memory(output, &bytes)?;
                context.set_return_u32(
                    u32::try_from(bytes.len()).map_err(|_| {
                        Win32Error::InvalidArgument("mmioRead result length overflow")
                    })?,
                );
            }
            Err(_) => context.set_return_u32(u32::MAX),
        }
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for MmioSeek {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        let offset = i64::from(context.argument_u32(1)? as i32);
        let origin = context.argument_u32(2)?;
        match context.seek_file(handle, offset, origin) {
            Ok(position) => context.set_return_u32(u32::try_from(position).unwrap_or(u32::MAX)),
            Err(_) => context.set_return_u32(u32::MAX),
        }
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

fn read_mmckinfo(
    context: &dyn HostCallContext,
    address: GuestAddress,
) -> Result<[u32; 5], Win32Error> {
    let mut bytes = [0; 20];
    context.read_memory(address, &mut bytes)?;
    let mut fields = [0; 5];
    for (field, source) in fields.iter_mut().zip(bytes.chunks_exact(4)) {
        *field = u32::from_le_bytes(
            source
                .try_into()
                .map_err(|_| Win32Error::InvalidArgument("MMCKINFO field"))?,
        );
    }
    Ok(fields)
}

fn write_mmckinfo(
    context: &mut dyn HostCallContext,
    address: GuestAddress,
    fields: [u32; 5],
) -> Result<(), Win32Error> {
    let mut bytes = [0; 20];
    for (target, field) in bytes.chunks_exact_mut(4).zip(fields) {
        target.copy_from_slice(&field.to_le_bytes());
    }
    context.write_memory(address, &bytes)
}

impl HostCallHandler for MmioAscend {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        let info = GuestAddress(context.argument_u32(1)?);
        let _flags = context.argument_u32(2)?;
        let fields = read_mmckinfo(context, info)?;
        let end = u64::from(fields[3]) + u64::from(fields[1]) + u64::from(fields[1] & 1);
        let result = i64::try_from(end)
            .ok()
            .and_then(|offset| context.seek_file(handle, offset, 0).ok())
            .map_or(MMIOERR_CANNOTSEEK, |_| 0);
        context.set_return_u32(result);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for MmioDescend {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let handle = Handle(context.argument_u32(0)?);
        let output = GuestAddress(context.argument_u32(1)?);
        let parent_address = GuestAddress(context.argument_u32(2)?);
        let flags = context.argument_u32(3)?;
        let requested = read_mmckinfo(context, output)?;
        let parent_end = if parent_address.0 == 0 {
            context.file_size(handle).unwrap_or(u64::MAX)
        } else {
            let parent = read_mmckinfo(context, parent_address)?;
            u64::from(parent[3]) + u64::from(parent[1])
        };

        let result = loop {
            let header_position = match context.seek_file(handle, 0, 1) {
                Ok(position) if position.saturating_add(8) <= parent_end => position,
                _ => break MMIOERR_CHUNKNOTFOUND,
            };
            let header = match context.read_file(handle, 8) {
                Ok(bytes) if bytes.len() == 8 => bytes,
                _ => break MMIOERR_CHUNKNOTFOUND,
            };
            let chunk_id = u32::from_le_bytes(
                header[..4]
                    .try_into()
                    .map_err(|_| Win32Error::InvalidArgument("MMCKINFO chunk identifier"))?,
            );
            let chunk_size = u32::from_le_bytes(
                header[4..]
                    .try_into()
                    .map_err(|_| Win32Error::InvalidArgument("MMCKINFO chunk length"))?,
            );
            let data_offset = header_position.saturating_add(8);
            let mut form_type = 0;
            if chunk_id == FOURCC_RIFF || chunk_id == FOURCC_LIST {
                let form = context.read_file(handle, 4).unwrap_or_default();
                if form.len() != 4 {
                    break MMIOERR_CHUNKNOTFOUND;
                }
                form_type = u32::from_le_bytes(
                    form.try_into()
                        .map_err(|_| Win32Error::InvalidArgument("MMCKINFO form type"))?,
                );
            }

            let matches = if flags & MMIO_FINDRIFF != 0 {
                chunk_id == FOURCC_RIFF && form_type == requested[2]
            } else if flags & MMIO_FINDLIST != 0 {
                chunk_id == FOURCC_LIST && form_type == requested[2]
            } else if flags & MMIO_FINDCHUNK != 0 {
                chunk_id == requested[0]
            } else {
                true
            };
            if matches {
                let data_offset = u32::try_from(data_offset)
                    .map_err(|_| Win32Error::InvalidArgument("MMCKINFO data offset overflow"))?;
                write_mmckinfo(
                    context,
                    output,
                    [chunk_id, chunk_size, form_type, data_offset, 0],
                )?;
                break 0;
            }

            let next = data_offset
                .saturating_add(u64::from(chunk_size))
                .saturating_add(u64::from(chunk_size & 1));
            let Some(next) = i64::try_from(next).ok() else {
                break MMIOERR_CANNOTSEEK;
            };
            if context.seek_file(handle, next, 0).is_err() {
                break MMIOERR_CANNOTSEEK;
            }
        };

        context.set_return_u32(result);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

impl HostCallHandler for JoyGetNumDevs {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        context.set_return_u32(0);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

impl HostCallHandler for WaveOutGetNumDevs {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        // Audio output is not connected to a Host backend yet. Reporting no
        // devices lets Guests take their documented silent-mode fallback and
        // avoids handing out a waveOut handle that cannot honor its contract.
        context.set_return_u32(0);
        context.set_stdcall_cleanup(0);
        Ok(())
    }
}

impl HostCallHandler for JoyUnavailable {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        for index in 0..(self.cleanup / 4) as usize {
            let _ = context.argument_u32(index)?;
        }
        context.set_return_u32(6); // MMSYSERR_NODRIVER
        context.set_stdcall_cleanup(self.cleanup);
        Ok(())
    }
}

impl HostCallHandler for MultimediaUnavailable {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        for index in 0..(self.cleanup / 4) as usize {
            let _ = context.argument_u32(index)?;
        }
        context.set_return_u32(self.result);
        context.set_stdcall_cleanup(self.cleanup);
        Ok(())
    }
}

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
        assert_eq!(registry.len(), 26);
    }
}
