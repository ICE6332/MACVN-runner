//! Headless command-line VNRT process runner.

use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use vnrt_gfx::GraphicsDevice;
use vnrt_runtime::{DiagnosticSnapshot, RunLimits, RunOutcome, Runtime, RuntimeConfig};
use vnrt_win32::{ApiRegistry, GuestAddress};

#[derive(Debug, Parser)]
#[command(about = "Load and interpret a PE32 executable with VNRT")]
struct Arguments {
    /// PE32 executable to run.
    path: PathBuf,
    /// Safety limit for this intentionally incomplete interpreter.
    #[arg(long, default_value_t = 1_000)]
    max_instructions: u64,
    /// Stop at the first Guest-presented frame and write it as PNG.
    #[arg(long, value_name = "PNG")]
    dump_first_frame: Option<PathBuf>,
}

fn main() -> Result<ExitCode> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let arguments = Arguments::parse();
    let bytes = fs::read(&arguments.path)
        .with_context(|| format!("failed to read {}", arguments.path.display()))?;

    let registry = build_registry();
    let file_name = arguments
        .path
        .file_name()
        .map_or_else(|| "guest.exe".into(), |name| name.to_string_lossy());
    let module_path = format!(r"C:\VNRT\{file_name}");
    let config = RuntimeConfig {
        command_line: format!(r#""{module_path}""#),
        module_path,
        filesystem_root: arguments
            .path
            .parent()
            .map_or_else(|| PathBuf::from("."), PathBuf::from),
        ..RuntimeConfig::default()
    };
    let mut runtime = Runtime::load_with_config(&bytes, registry, config)
        .with_context(|| format!("failed to load {}", arguments.path.display()))?;
    runtime
        .memory
        .set_track_executable_writes(std::env::var_os("VNRT_TRACK_EXEC_WRITES").is_some());
    if let Some(range) = std::env::var_os("VNRT_TRACK_WRITES") {
        runtime
            .memory
            .set_tracked_write_range(Some(parse_address_range(&range.to_string_lossy())?));
    }
    if let Some(range) = std::env::var_os("VNRT_TRACE_INSTRUCTIONS") {
        runtime
            .cpu
            .set_trace_range(Some(parse_address_range(&range.to_string_lossy())?));
    }
    runtime
        .cpu
        .set_block_profiling(std::env::var_os("VNRT_PROFILE_BLOCKS").is_some());
    let graphics = vnrt_gfx_wgpu::WgpuGraphicsDevice::new().context("failed to initialize GPU")?;
    info!(adapter = graphics.adapter_name(), "Host GPU initialized");
    runtime.set_graphics_device(Box::new(graphics));

    info!(path = %arguments.path.display(), "guest image loaded");
    let limits = RunLimits {
        max_instructions: arguments.max_instructions,
    };
    let outcome = match if arguments.dump_first_frame.is_some() {
        runtime.run_until_first_frame(limits)
    } else {
        runtime.run(limits)
    } {
        Ok(outcome) => outcome,
        Err(runtime_error) => {
            let snapshot = runtime.diagnostic_snapshot();
            let fault_code_window = memory_window(&runtime, snapshot.registers.eip, 48, 80);
            error!(
                error = %runtime_error,
                registers = %vnrt_debugger::format_registers(&snapshot.registers),
                fs_base = format_args!("{:#010x}", snapshot.fs_base),
                instruction_bytes = %format_bytes(&snapshot.instruction_bytes),
                fault_code_window,
                stack_words = ?snapshot.stack_words,
                stack_pointer_previews = ?stack_pointer_previews(&runtime, &snapshot.stack_words),
                exception_chain = ?snapshot.exception_chain,
                recent_host_calls = ?snapshot.recent_host_calls,
                recent_control_transfers = ?snapshot.recent_control_transfers.iter().rev().take(32).collect::<Vec<_>>(),
                control_transfer_previews = ?control_transfer_previews(&runtime, &snapshot),
                matching_stack_transfers = ?matching_stack_transfers(&runtime, &snapshot),
                recent_executable_writes = ?runtime.memory.executable_writes().iter().rev().take(64).collect::<Vec<_>>(),
                executable_write_source_previews = ?executable_write_source_previews(&runtime),
                traced_instructions = ?runtime.cpu.traced_instructions(),
                targeted_control_transfers = ?runtime.cpu.targeted_control_transfers(),
                hottest_blocks = ?runtime.cpu.hottest_blocks(32),
                "guest execution failed"
            );
            emit_guest_output(&runtime)?;
            return Err(runtime_error.into());
        }
    };
    if let RunOutcome::FramePresented(window) = outcome {
        let path = arguments
            .dump_first_frame
            .as_ref()
            .context("frame stop requested without an output path")?;
        let frame = runtime
            .window_frame(window)
            .context("presented window does not retain its frame")?;
        image::save_buffer_with_format(
            path,
            &frame.rgba,
            frame.width,
            frame.height,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .with_context(|| format!("failed to write {}", path.display()))?;
        info!(window, path = %path.display(), "first Guest frame written");
    }
    emit_guest_output(&runtime)?;
    if std::env::var_os("VNRT_TRACE_INSTRUCTIONS").is_some() {
        info!(
            traced_instructions = ?runtime.cpu.traced_instructions(),
            "Guest instruction trace completed"
        );
    }
    if std::env::var_os("VNRT_PROFILE_BLOCKS").is_some() {
        info!(hottest_blocks = ?runtime.cpu.hottest_blocks(32), "Guest block profile completed");
    }
    if let Some(range) = std::env::var_os("VNRT_DUMP_MEMORY") {
        let (start, end) = parse_address_range(&range.to_string_lossy())?;
        let length = usize::try_from(end - start).context("memory dump range is too large")?;
        info!(
            memory = %memory_window(&runtime, start, 0, length),
            "Guest memory dump completed"
        );
    }
    info!(?outcome, "guest stopped");
    Ok(match outcome {
        RunOutcome::Exited(code) => ExitCode::from(code.to_le_bytes()[0]),
        RunOutcome::Halted | RunOutcome::FramePresented(_) => ExitCode::SUCCESS,
    })
}

fn build_registry() -> ApiRegistry {
    let mut registry = ApiRegistry::new();
    vnrt_advapi32::register(&mut registry);
    vnrt_comctl32::register(&mut registry);
    vnrt_d3d9::register(&mut registry);
    vnrt_dsound::register(&mut registry);
    vnrt_gdi32::register(&mut registry);
    vnrt_imm32::register(&mut registry);
    vnrt_kernel32::register(&mut registry);
    vnrt_logprint::register(&mut registry);
    vnrt_ntdll::register(&mut registry);
    vnrt_ole32::register(&mut registry);
    vnrt_psapi::register(&mut registry);
    vnrt_shell32::register(&mut registry);
    vnrt_user32::register(&mut registry);
    vnrt_version::register(&mut registry);
    vnrt_winmm::register(&mut registry);
    registry
}

fn memory_window(runtime: &Runtime, center: u32, before: u32, length: usize) -> String {
    let start = center.saturating_sub(before);
    let mut bytes = vec![0_u8; length];
    runtime
        .memory
        .read(GuestAddress(start), &mut bytes)
        .map_or_else(
            |_| format!("{start:#010x}: <unmapped>"),
            |_| format!("{start:#010x}: {}", format_bytes(&bytes)),
        )
}

fn parse_address_range(value: &str) -> Result<(u32, u32)> {
    let (start, end) = value
        .split_once('-')
        .context("VNRT_TRACK_WRITES must be START-END")?;
    let parse = |part: &str| {
        u32::from_str_radix(part.trim_start_matches("0x"), 16)
            .with_context(|| format!("invalid Guest address {part}"))
    };
    let range = (parse(start)?, parse(end)?);
    anyhow::ensure!(range.0 < range.1, "tracked write range must not be empty");
    Ok(range)
}

fn stack_pointer_previews(runtime: &Runtime, words: &[u32]) -> Vec<(u32, String)> {
    words
        .iter()
        .copied()
        .filter_map(|address| {
            let mut bytes = [0_u8; 64];
            runtime
                .memory
                .read(GuestAddress(address), &mut bytes)
                .ok()?;
            let ascii = bytes
                .iter()
                .take_while(|byte| **byte != 0)
                .map(|byte| {
                    if byte.is_ascii_graphic() || *byte == b' ' {
                        char::from(*byte)
                    } else {
                        '.'
                    }
                })
                .collect::<String>();
            let utf16 = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .take_while(|unit| *unit != 0)
                .collect::<Vec<_>>();
            let utf16 = String::from_utf16(&utf16).unwrap_or_default();
            (!ascii.is_empty() || !utf16.is_empty())
                .then(|| (address, format!("ascii={ascii:?}, utf16={utf16:?}")))
        })
        .collect()
}

fn control_transfer_previews(runtime: &Runtime, snapshot: &DiagnosticSnapshot) -> Vec<String> {
    snapshot
        .recent_control_transfers
        .iter()
        .rev()
        .take(6)
        .rev()
        .map(|transfer| {
            let start = transfer.source.saturating_sub(16);
            let mut code = [0_u8; 40];
            let code = runtime
                .memory
                .read(GuestAddress(start), &mut code)
                .map_or_else(|_| "<unmapped>".to_owned(), |_| format_bytes(&code));
            let stack = (0..4_u32)
                .filter_map(|index| {
                    transfer
                        .stack_pointer
                        .checked_add(index * 4)
                        .and_then(|address| runtime.memory.read_u32(GuestAddress(address)).ok())
                })
                .collect::<Vec<_>>();
            format!(
                "{:?} {:#010x}->{:#010x} esp={:#010x} code@{:#010x}=[{}] stack={stack:?}",
                transfer.kind,
                transfer.source,
                transfer.target,
                transfer.stack_pointer,
                start,
                code,
            )
        })
        .collect()
}

fn matching_stack_transfers(runtime: &Runtime, snapshot: &DiagnosticSnapshot) -> Vec<String> {
    snapshot
        .recent_control_transfers
        .iter()
        .filter(|transfer| transfer.stack_pointer == snapshot.registers.esp)
        .map(|transfer| {
            let start = transfer.source.saturating_sub(12);
            let mut code = [0_u8; 32];
            let code = runtime
                .memory
                .read(GuestAddress(start), &mut code)
                .map_or_else(|_| "<unmapped>".to_owned(), |_| format_bytes(&code));
            format!(
                "{:?} {:#010x}->{:#010x} code@{:#010x}=[{}]",
                transfer.kind, transfer.source, transfer.target, start, code
            )
        })
        .collect()
}

fn executable_write_source_previews(runtime: &Runtime) -> Vec<String> {
    runtime
        .memory
        .executable_writes()
        .iter()
        .rev()
        .filter_map(|write| write.source)
        .take(16)
        .map(|source| memory_window(runtime, source.0, 24, 64))
        .collect()
}

fn emit_guest_output(runtime: &Runtime) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(runtime.guest_stdout())?;
    stdout.flush()?;
    let mut stderr = io::stderr().lock();
    stderr.write_all(runtime.guest_stderr())?;
    stderr.flush()
}

fn format_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_gdi_export_dispatches_to_stretch_dibits() {
        let image = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/guest-programs/exit42.exe"
        ));
        let runtime = Runtime::load(image, build_registry()).expect("runtime should load");
        let module = runtime
            .host_module_handle("gdi32.dll")
            .expect("synthetic gdi32 should exist");
        let address = find_export(&runtime, module, "StretchDIBits");
        assert_eq!(
            runtime.host_api_at(address),
            Some(&vnrt_win32::ApiKey::new("gdi32.dll", "StretchDIBits"))
        );
    }

    fn find_export(runtime: &Runtime, module: GuestAddress, target: &str) -> GuestAddress {
        let pe = module.0 + 0x80;
        let directory = module.0 + runtime.memory.read_u32(GuestAddress(pe + 0x78)).unwrap();
        let name_count = runtime
            .memory
            .read_u32(GuestAddress(directory + 24))
            .unwrap();
        let functions = module.0
            + runtime
                .memory
                .read_u32(GuestAddress(directory + 28))
                .unwrap();
        let names = module.0
            + runtime
                .memory
                .read_u32(GuestAddress(directory + 32))
                .unwrap();
        let ordinals = module.0
            + runtime
                .memory
                .read_u32(GuestAddress(directory + 36))
                .unwrap();
        for index in 0..name_count {
            let name_rva = runtime
                .memory
                .read_u32(GuestAddress(names + index * 4))
                .unwrap();
            let mut bytes = Vec::new();
            for offset in 0..256 {
                let mut byte = [0];
                runtime
                    .memory
                    .read(GuestAddress(module.0 + name_rva + offset), &mut byte)
                    .unwrap();
                if byte[0] == 0 {
                    break;
                }
                bytes.push(byte[0]);
            }
            if bytes == target.as_bytes() {
                let ordinal = u32::from(
                    runtime
                        .memory
                        .read_u16(GuestAddress(ordinals + index * 2))
                        .unwrap(),
                );
                let function_rva = runtime
                    .memory
                    .read_u32(GuestAddress(functions + ordinal * 4))
                    .unwrap();
                return GuestAddress(module.0 + function_rva);
            }
        }
        panic!("missing synthetic export {target}");
    }
}
