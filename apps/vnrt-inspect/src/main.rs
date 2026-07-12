//! Command-line PE32 metadata inspector.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Parser;
use iced_x86::{Decoder, DecoderOptions, Instruction, Mnemonic, OpKind, Register};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(about = "Inspect the PE32 metadata currently understood by VNRT")]
struct Arguments {
    /// PE32 executable to inspect.
    path: PathBuf,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
    /// Decode executable sections and report instruction forms and x87 gaps.
    #[arg(long)]
    instruction_census: bool,
    /// Report indirect COM-vtable call offsets and possible D3D9 methods.
    #[arg(long)]
    d3d9_census: bool,
    /// Run both instruction and D3D9 censuses.
    #[arg(long)]
    census: bool,
}

#[derive(Debug, Serialize)]
struct Census {
    decoded_instructions: u64,
    invalid_instructions: u64,
    forms: BTreeMap<String, u64>,
    x87_forms: BTreeMap<String, X87Form>,
    com_vtable_calls: BTreeMap<u32, ComCall>,
}

#[derive(Debug, Serialize)]
struct X87Form {
    count: u64,
    support: &'static str,
}

#[derive(Debug, Serialize)]
struct ComCall {
    count: u64,
    possible_d3d9_methods: Vec<&'static str>,
}

fn main() -> Result<()> {
    let arguments = Arguments::parse();
    let bytes = fs::read(&arguments.path)
        .with_context(|| format!("failed to read {}", arguments.path.display()))?;
    let image = vnrt_pe::parse(&bytes)
        .with_context(|| format!("failed to parse {}", arguments.path.display()))?;

    let wants_instruction_census = arguments.census || arguments.instruction_census;
    let wants_d3d9_census = arguments.census || arguments.d3d9_census;
    if wants_instruction_census || wants_d3d9_census {
        let census = census(&bytes, &image, wants_instruction_census, wants_d3d9_census)?;
        if arguments.json {
            println!("{}", serde_json::to_string_pretty(&census)?);
        } else {
            print_census(
                &arguments.path,
                &census,
                wants_instruction_census,
                wants_d3d9_census,
            );
        }
    } else if arguments.json {
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

fn census(
    bytes: &[u8],
    image: &vnrt_pe::PeImage,
    include_instructions: bool,
    include_d3d9: bool,
) -> Result<Census> {
    let mut result = Census {
        decoded_instructions: 0,
        invalid_instructions: 0,
        forms: BTreeMap::new(),
        x87_forms: BTreeMap::new(),
        com_vtable_calls: BTreeMap::new(),
    };
    for section in image
        .sections
        .iter()
        .filter(|section| section.characteristics & 0x2000_0000 != 0)
    {
        let raw_size = section.size_of_raw_data.min(section.virtual_size.max(1));
        let offset = image
            .file_offset_for_rva(section.virtual_address, raw_size)
            .with_context(|| format!("failed to locate executable section {}", section.name))?;
        let size = usize::try_from(raw_size).context("section size does not fit usize")?;
        let section_bytes = bytes
            .get(offset..offset + size)
            .context("executable section extends beyond the input")?;
        let ip = u64::from(image.optional.image_base) + u64::from(section.virtual_address);
        let mut decoder = Decoder::with_ip(32, section_bytes, ip, DecoderOptions::NONE);
        while decoder.can_decode() {
            let instruction = decoder.decode();
            if instruction.is_invalid() {
                result.invalid_instructions += 1;
                continue;
            }
            result.decoded_instructions += 1;
            if include_instructions {
                let form = instruction_form(&instruction);
                *result.forms.entry(form.clone()).or_default() += 1;
                if is_x87(instruction.mnemonic()) {
                    let support = x87_support(&instruction);
                    let entry = result
                        .x87_forms
                        .entry(form)
                        .or_insert(X87Form { count: 0, support });
                    entry.count += 1;
                }
            }
            if include_d3d9
                && instruction.mnemonic() == Mnemonic::Call
                && instruction.op0_kind() == OpKind::Memory
                && is_general_register(instruction.memory_base())
                && instruction.memory_index() == Register::None
                && instruction.memory_displacement32() <= 0x200
            {
                let offset = instruction.memory_displacement32();
                let entry = result
                    .com_vtable_calls
                    .entry(offset)
                    .or_insert_with(|| ComCall {
                        count: 0,
                        possible_d3d9_methods: d3d9_methods(offset),
                    });
                entry.count += 1;
            }
        }
    }
    Ok(result)
}

fn instruction_form(instruction: &Instruction) -> String {
    let mut operands = Vec::new();
    for index in 0..instruction.op_count() {
        let kind = instruction.op_kind(index);
        let operand = match kind {
            OpKind::Register => format!("{:?}", instruction.op_register(index)),
            OpKind::Memory => format!("mem:{:?}", instruction.memory_size()),
            _ => format!("{kind:?}"),
        };
        operands.push(operand);
    }
    if operands.is_empty() {
        format!("{:?}", instruction.mnemonic())
    } else {
        format!("{:?} {}", instruction.mnemonic(), operands.join(","))
    }
}

fn is_x87(mnemonic: Mnemonic) -> bool {
    let name = format!("{mnemonic:?}");
    name.starts_with('F') || mnemonic == Mnemonic::Wait
}

fn x87_support(instruction: &Instruction) -> &'static str {
    use Mnemonic as M;
    match instruction.mnemonic() {
        M::Wait
        | M::Fnclex
        | M::Fnstcw
        | M::Fldcw
        | M::Fnstsw
        | M::Fstsw
        | M::Fsqrt
        | M::Frndint
        | M::Fyl2x
        | M::F2xm1
        | M::Fscale
        | M::Fld1
        | M::Fldz
        | M::Fldpi
        | M::Fldl2e
        | M::Fldl2t
        | M::Fldlg2
        | M::Fldln2
        | M::Fabs
        | M::Fchs
        | M::Fxch => "implemented",
        M::Fadd
        | M::Faddp
        | M::Fsub
        | M::Fsubp
        | M::Fsubr
        | M::Fsubrp
        | M::Fmul
        | M::Fmulp
        | M::Fdiv
        | M::Fdivp
        | M::Fdivr
        | M::Fdivrp => {
            if instruction.op_count() <= 2 {
                "implemented"
            } else {
                "unsupported-form"
            }
        }
        M::Fcom | M::Fcomp | M::Fcompp | M::Fucom | M::Fucomp | M::Fucompp => {
            match instruction.memory_size() {
                iced_x86::MemorySize::Float32
                | iced_x86::MemorySize::Float64
                | iced_x86::MemorySize::Unknown => "implemented",
                _ => "unsupported-form",
            }
        }
        M::Fild => match instruction.memory_size() {
            iced_x86::MemorySize::Int16
            | iced_x86::MemorySize::Int32
            | iced_x86::MemorySize::Int64 => "implemented",
            _ => "unsupported-form",
        },
        M::Fld => {
            if instruction.op0_kind() == OpKind::Register
                || matches!(
                    instruction.memory_size(),
                    iced_x86::MemorySize::Float32 | iced_x86::MemorySize::Float64
                )
            {
                "implemented"
            } else {
                "unsupported-form"
            }
        }
        M::Fst | M::Fstp => {
            if instruction.op0_kind() == OpKind::Register
                || matches!(
                    instruction.memory_size(),
                    iced_x86::MemorySize::Float32 | iced_x86::MemorySize::Float64
                )
            {
                "implemented"
            } else {
                "unsupported-form"
            }
        }
        M::Fist | M::Fistp | M::Fisttp => match instruction.memory_size() {
            iced_x86::MemorySize::Int16
            | iced_x86::MemorySize::Int32
            | iced_x86::MemorySize::Int64 => "implemented",
            _ => "unsupported-form",
        },
        _ => "missing",
    }
}

fn is_general_register(register: Register) -> bool {
    matches!(
        register,
        Register::EAX
            | Register::EBX
            | Register::ECX
            | Register::EDX
            | Register::ESI
            | Register::EDI
            | Register::EBP
            | Register::ESP
    )
}

fn d3d9_methods(offset: u32) -> Vec<&'static str> {
    match offset {
        0x00 => vec!["IUnknown::QueryInterface"],
        0x04 => vec!["IUnknown::AddRef"],
        0x08 => vec!["IUnknown::Release"],
        0x0c => vec!["IDirect3DDevice9::TestCooperativeLevel"],
        0x40 => vec!["IDirect3D9::CreateDevice", "IDirect3DDevice9::Reset"],
        0x44 => vec!["IDirect3DDevice9::Present"],
        0x48 => vec!["IDirect3DTexture9::GetSurfaceLevel"],
        0x4c => vec!["IDirect3DTexture9::LockRect"],
        0x50 => vec!["IDirect3DTexture9::UnlockRect"],
        0x5c => vec!["IDirect3DDevice9::CreateTexture"],
        0xa4 => vec!["IDirect3DDevice9::BeginScene"],
        0xa8 => vec!["IDirect3DDevice9::EndScene"],
        0xac => vec!["IDirect3DDevice9::Clear"],
        0xe4 => vec!["IDirect3DDevice9::SetRenderState"],
        0x104 => vec!["IDirect3DDevice9::SetTexture"],
        0x10c => vec!["IDirect3DDevice9::SetTextureStageState"],
        0x114 => vec!["IDirect3DDevice9::SetSamplerState"],
        0x144 => vec!["IDirect3DDevice9::DrawPrimitive"],
        0x148 => vec!["IDirect3DDevice9::DrawIndexedPrimitive"],
        0x14c => vec!["IDirect3DDevice9::DrawPrimitiveUP"],
        0x150 => vec!["IDirect3DDevice9::DrawIndexedPrimitiveUP"],
        0x164 => vec!["IDirect3DDevice9::SetFVF"],
        0x190 => vec!["IDirect3DDevice9::SetStreamSource"],
        0x1a0 => vec!["IDirect3DDevice9::SetIndices"],
        _ => Vec::new(),
    }
}

fn print_census(path: &Path, census: &Census, instructions: bool, d3d9: bool) {
    println!("PE32 census: {}", path.display());
    println!("  decoded instructions: {}", census.decoded_instructions);
    println!("  invalid bytes/forms:  {}", census.invalid_instructions);
    if instructions {
        let missing: Vec<_> = census
            .x87_forms
            .iter()
            .filter(|(_, form)| form.support != "implemented")
            .collect();
        println!("  unique forms:         {}", census.forms.len());
        println!("  unique x87 forms:     {}", census.x87_forms.len());
        println!("  x87 gaps:             {}", missing.len());
        for (form, details) in missing {
            println!(
                "    {:<18} x{:>6}  {}",
                details.support, details.count, form
            );
        }
    }
    if d3d9 {
        println!("  COM-vtable candidates:");
        for (offset, call) in &census.com_vtable_calls {
            let names = if call.possible_d3d9_methods.is_empty() {
                "unmapped".to_owned()
            } else {
                call.possible_d3d9_methods.join(" | ")
            };
            println!("    +{offset:#05x} x{:>6}  {names}", call.count);
        }
    }
}
