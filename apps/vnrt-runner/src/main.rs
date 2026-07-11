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
use vnrt_runtime::{RunLimits, RunOutcome, Runtime, RuntimeConfig};
use vnrt_win32::{ApiRegistry, GuestAddress};

#[derive(Debug, Parser)]
#[command(about = "Load and interpret a PE32 executable with VNRT")]
struct Arguments {
    /// PE32 executable to run.
    path: PathBuf,
    /// Safety limit for this intentionally incomplete interpreter.
    #[arg(long, default_value_t = 1_000)]
    max_instructions: u64,
}

fn main() -> Result<ExitCode> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let arguments = Arguments::parse();
    let bytes = fs::read(&arguments.path)
        .with_context(|| format!("failed to read {}", arguments.path.display()))?;

    let mut registry = ApiRegistry::new();
    vnrt_advapi32::register(&mut registry);
    vnrt_gdi32::register(&mut registry);
    vnrt_kernel32::register(&mut registry);
    vnrt_ntdll::register(&mut registry);
    vnrt_ole32::register(&mut registry);
    vnrt_shell32::register(&mut registry);
    vnrt_user32::register(&mut registry);
    vnrt_winmm::register(&mut registry);
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

    info!(path = %arguments.path.display(), "guest image loaded");
    let outcome = match runtime.run(RunLimits {
        max_instructions: arguments.max_instructions,
    }) {
        Ok(outcome) => outcome,
        Err(runtime_error) => {
            let snapshot = runtime.diagnostic_snapshot();
            error!(
                error = %runtime_error,
                registers = %vnrt_debugger::format_registers(&snapshot.registers),
                fs_base = format_args!("{:#010x}", snapshot.fs_base),
                instruction_bytes = %format_bytes(&snapshot.instruction_bytes),
                stack_words = ?snapshot.stack_words,
                stack_pointer_previews = ?stack_pointer_previews(&runtime, &snapshot.stack_words),
                exception_chain = ?snapshot.exception_chain,
                recent_host_calls = ?snapshot.recent_host_calls,
                "guest execution failed"
            );
            emit_guest_output(&runtime)?;
            return Err(runtime_error.into());
        }
    };
    emit_guest_output(&runtime)?;
    info!(?outcome, "guest stopped");
    Ok(match outcome {
        RunOutcome::Exited(code) => ExitCode::from(code.to_le_bytes()[0]),
        RunOutcome::Halted => ExitCode::SUCCESS,
    })
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
