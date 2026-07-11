//! Command-line PE32 metadata inspector.

use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Inspect the PE32 metadata currently understood by VNRT")]
struct Arguments {
    /// PE32 executable to inspect.
    path: PathBuf,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

fn main() -> Result<()> {
    let arguments = Arguments::parse();
    let bytes = fs::read(&arguments.path)
        .with_context(|| format!("failed to read {}", arguments.path.display()))?;
    let image = vnrt_pe::parse(&bytes)
        .with_context(|| format!("failed to parse {}", arguments.path.display()))?;

    if arguments.json {
        println!("{}", serde_json::to_string_pretty(&image)?);
    } else {
        println!("PE32: {}", arguments.path.display());
        println!("  image base:  {:#010x}", image.optional.image_base);
        println!("  entry point: {:#010x}", image.entry_point());
        println!("  image size:  {:#x}", image.optional.size_of_image);
        println!("  sections:");
        for section in &image.sections {
            println!(
                "    {:<8} RVA={:#010x} virtual={:#x} raw={:#x}",
                section.name,
                section.virtual_address,
                section.virtual_size,
                section.size_of_raw_data
            );
        }
        println!("  imports:     {}", image.imports.len());
        for import in &image.imports {
            let symbol = import
                .name
                .clone()
                .unwrap_or_else(|| format!("#{}", import.ordinal.unwrap_or(0)));
            println!(
                "    {}!{} IAT_RVA={:#010x}",
                import.module, symbol, import.iat_rva
            );
        }
        println!("  relocations: {}", image.relocations.len());
        if let Some(tls) = image.tls {
            println!(
                "  TLS:          template={:#010x}..{:#010x} zero_fill={:#x}",
                tls.start_address_of_raw_data, tls.end_address_of_raw_data, tls.size_of_zero_fill
            );
        }
    }
    Ok(())
}
