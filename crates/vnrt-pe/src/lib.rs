//! Minimal, dependency-light PE32 parsing for the runtime loader.

use scroll::{LE, Pread};
use serde::Serialize;
use thiserror::Error;

const DOS_MAGIC: u16 = 0x5a4d;
const PE_SIGNATURE: u32 = 0x0000_4550;
const I386_MACHINE: u16 = 0x014c;
const PE32_MAGIC: u16 = 0x010b;
const SECTION_HEADER_SIZE: usize = 40;
const OPTIONAL_HEADER_DATA_DIRECTORIES_OFFSET: usize = 96;
const DATA_DIRECTORY_SIZE: usize = 8;
const MAX_DATA_DIRECTORIES: usize = 16;
const IMPORT_DIRECTORY_INDEX: usize = 1;
const RELOCATION_DIRECTORY_INDEX: usize = 5;
const TLS_DIRECTORY_INDEX: usize = 9;
const IMPORT_DESCRIPTOR_SIZE: u32 = 20;
const RELOCATION_BLOCK_HEADER_SIZE: u32 = 8;
const ORDINAL_FLAG32: u32 = 0x8000_0000;

/// Errors produced while inspecting or loading a PE image.
#[derive(Debug, Error)]
pub enum PeError {
    /// The file does not contain the required PE structure.
    #[error("invalid PE image: {0}")]
    Invalid(&'static str),
    /// A valid PE feature is outside the deliberately small supported subset.
    #[error("unsupported PE feature: {0}")]
    Unsupported(&'static str),
    /// A field points outside the supplied file.
    #[error("PE data is truncated at file offset {offset:#x}")]
    Truncated {
        /// File offset at which a complete primitive was expected.
        offset: usize,
    },
    /// An RVA does not resolve to bytes backed by the file.
    #[error("PE RVA range {rva:#010x}..+{size:#x} is not file-backed")]
    UnmappedRva {
        /// Starting relative virtual address.
        rva: u32,
        /// Required byte count.
        size: u32,
    },
    /// A primitive field could not be decoded.
    #[error("failed to read PE field: {0}")]
    Read(#[from] scroll::Error),
}

/// COFF metadata used to locate sections and validate the guest architecture.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CoffHeader {
    /// Target machine identifier. Only `IMAGE_FILE_MACHINE_I386` is accepted.
    pub machine: u16,
    /// Number of section table entries.
    pub number_of_sections: u16,
    /// Size of the optional header following this structure.
    pub size_of_optional_header: u16,
    /// COFF image characteristics.
    pub characteristics: u16,
}

/// One optional-header data-directory location.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
pub struct DataDirectory {
    /// Relative virtual address of the directory.
    pub rva: u32,
    /// Directory byte size.
    pub size: u32,
}

impl DataDirectory {
    /// Whether the directory is absent.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.rva == 0 || self.size == 0
    }
}

/// PE32 optional-header fields needed by the initial image loader.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OptionalHeader32 {
    /// Initial instruction pointer relative to the image base.
    pub address_of_entry_point: u32,
    /// Preferred 32-bit load base.
    pub image_base: u32,
    /// In-memory section alignment.
    pub section_alignment: u32,
    /// On-disk section alignment.
    pub file_alignment: u32,
    /// Total mapped image size.
    pub size_of_image: u32,
    /// File headers to copy into guest memory.
    pub size_of_headers: u32,
    /// Number of data-directory slots declared by the image.
    pub number_of_rva_and_sizes: u32,
    /// Data directories present in the optional header, capped at the PE32 standard 16.
    pub data_directories: Vec<DataDirectory>,
}

/// A raw PE section-table entry. Data is copied by the runtime loader.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Section {
    /// Human-readable, NUL-trimmed section name.
    pub name: String,
    /// Size required once mapped.
    pub virtual_size: u32,
    /// RVA at which the section is mapped.
    pub virtual_address: u32,
    /// Bytes stored in the file.
    pub size_of_raw_data: u32,
    /// File offset of the raw section bytes.
    pub pointer_to_raw_data: u32,
    /// Section access and content flags.
    pub characteristics: u32,
}

/// An imported symbol resolved from the import lookup table.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Import {
    /// DLL name as stored by the image.
    pub module: String,
    /// Symbol name, when imported by name.
    pub name: Option<String>,
    /// Ordinal, when imported by ordinal.
    pub ordinal: Option<u16>,
    /// RVA of the import address table slot patched by the runtime.
    pub iat_rva: u32,
}

/// A base-relocation fixup.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Relocation {
    /// RVA of the value to adjust.
    pub rva: u32,
    /// PE relocation kind.
    pub kind: u8,
}

/// PE32 static thread-local-storage directory.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct TlsDirectory32 {
    /// Inclusive virtual address of initialized TLS template bytes.
    pub start_address_of_raw_data: u32,
    /// Exclusive virtual address of initialized TLS template bytes.
    pub end_address_of_raw_data: u32,
    /// Virtual address where the loader writes this module's TLS slot index.
    pub address_of_index: u32,
    /// Virtual address of a null-terminated TLS callback pointer array.
    pub address_of_callbacks: u32,
    /// Additional zero-initialized bytes following the copied template.
    pub size_of_zero_fill: u32,
    /// Alignment and platform characteristics supplied by the linker.
    pub characteristics: u32,
}

/// Parsed PE32 metadata used by the loader and inspection tools.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PeImage {
    /// COFF file header.
    pub coff: CoffHeader,
    /// PE32 optional header.
    pub optional: OptionalHeader32,
    /// Section table.
    pub sections: Vec<Section>,
    /// Imported functions.
    pub imports: Vec<Import>,
    /// Base relocation fixups.
    pub relocations: Vec<Relocation>,
    /// Static TLS metadata, when the image declares it.
    pub tls: Option<TlsDirectory32>,
}

impl PeImage {
    /// Returns the preferred absolute entry-point address.
    #[must_use]
    pub const fn entry_point(&self) -> u32 {
        self.optional
            .image_base
            .wrapping_add(self.optional.address_of_entry_point)
    }

    /// Translate a file-backed RVA range to its file offset.
    pub fn file_offset_for_rva(&self, rva: u32, size: u32) -> Result<usize, PeError> {
        file_offset_for_rva(rva, size, self.optional.size_of_headers, &self.sections)
    }
}

/// Parse PE32 headers, sections, imports, and base relocations.
pub fn parse(bytes: &[u8]) -> Result<PeImage, PeError> {
    if read_u16(bytes, 0)? != DOS_MAGIC {
        return Err(PeError::Invalid("missing MZ signature"));
    }

    let pe_offset = usize::try_from(read_u32(bytes, 0x3c)?)
        .map_err(|_| PeError::Invalid("PE header offset does not fit host usize"))?;
    if read_u32(bytes, pe_offset)? != PE_SIGNATURE {
        return Err(PeError::Invalid("missing PE signature"));
    }

    let coff_offset = checked_add(pe_offset, 4)?;
    let machine = read_u16(bytes, coff_offset)?;
    if machine != I386_MACHINE {
        return Err(PeError::Unsupported("only 32-bit i386 images are accepted"));
    }

    let number_of_sections = read_u16(bytes, checked_add(coff_offset, 2)?)?;
    let size_of_optional_header = read_u16(bytes, checked_add(coff_offset, 16)?)?;
    let characteristics = read_u16(bytes, checked_add(coff_offset, 18)?)?;
    let optional_offset = checked_add(coff_offset, 20)?;
    if read_u16(bytes, optional_offset)? != PE32_MAGIC {
        return Err(PeError::Unsupported(
            "only PE32 optional headers are accepted",
        ));
    }
    if usize::from(size_of_optional_header) < OPTIONAL_HEADER_DATA_DIRECTORIES_OFFSET {
        return Err(PeError::Invalid("PE32 optional header is too small"));
    }

    let number_of_rva_and_sizes = read_u32(bytes, checked_add(optional_offset, 92)?)?;
    let directory_count = usize::try_from(number_of_rva_and_sizes)
        .unwrap_or(usize::MAX)
        .min(MAX_DATA_DIRECTORIES);
    let directory_bytes = directory_count
        .checked_mul(DATA_DIRECTORY_SIZE)
        .ok_or(PeError::Invalid("data-directory size overflow"))?;
    if OPTIONAL_HEADER_DATA_DIRECTORIES_OFFSET + directory_bytes
        > usize::from(size_of_optional_header)
    {
        return Err(PeError::Invalid(
            "data directories exceed the optional header",
        ));
    }
    let mut data_directories = Vec::with_capacity(directory_count);
    for index in 0..directory_count {
        let offset = checked_add(
            optional_offset,
            OPTIONAL_HEADER_DATA_DIRECTORIES_OFFSET + index * DATA_DIRECTORY_SIZE,
        )?;
        data_directories.push(DataDirectory {
            rva: read_u32(bytes, offset)?,
            size: read_u32(bytes, checked_add(offset, 4)?)?,
        });
    }

    let optional = OptionalHeader32 {
        address_of_entry_point: read_u32(bytes, checked_add(optional_offset, 16)?)?,
        image_base: read_u32(bytes, checked_add(optional_offset, 28)?)?,
        section_alignment: read_u32(bytes, checked_add(optional_offset, 32)?)?,
        file_alignment: read_u32(bytes, checked_add(optional_offset, 36)?)?,
        size_of_image: read_u32(bytes, checked_add(optional_offset, 56)?)?,
        size_of_headers: read_u32(bytes, checked_add(optional_offset, 60)?)?,
        number_of_rva_and_sizes,
        data_directories,
    };

    let section_table = checked_add(optional_offset, usize::from(size_of_optional_header))?;
    let mut sections = Vec::with_capacity(usize::from(number_of_sections));
    for index in 0..usize::from(number_of_sections) {
        let offset = checked_add(section_table, index * SECTION_HEADER_SIZE)?;
        let name_bytes = bytes
            .get(offset..checked_add(offset, 8)?)
            .ok_or(PeError::Truncated { offset })?;
        let name_len = name_bytes.iter().position(|byte| *byte == 0).unwrap_or(8);
        let name = String::from_utf8_lossy(&name_bytes[..name_len]).into_owned();
        sections.push(Section {
            name,
            virtual_size: read_u32(bytes, checked_add(offset, 8)?)?,
            virtual_address: read_u32(bytes, checked_add(offset, 12)?)?,
            size_of_raw_data: read_u32(bytes, checked_add(offset, 16)?)?,
            pointer_to_raw_data: read_u32(bytes, checked_add(offset, 20)?)?,
            characteristics: read_u32(bytes, checked_add(offset, 36)?)?,
        });
    }

    let imports = parse_imports(bytes, &optional, &sections)?;
    let relocations = parse_relocations(bytes, &optional, &sections)?;
    let tls = parse_tls_directory(bytes, &optional, &sections)?;
    Ok(PeImage {
        coff: CoffHeader {
            machine,
            number_of_sections,
            size_of_optional_header,
            characteristics,
        },
        optional,
        sections,
        imports,
        relocations,
        tls,
    })
}

fn parse_tls_directory(
    bytes: &[u8],
    optional: &OptionalHeader32,
    sections: &[Section],
) -> Result<Option<TlsDirectory32>, PeError> {
    let Some(directory) = optional.data_directories.get(TLS_DIRECTORY_INDEX).copied() else {
        return Ok(None);
    };
    if directory.is_empty() {
        return Ok(None);
    }
    if directory.size < 24 {
        return Err(PeError::Invalid("truncated PE32 TLS directory"));
    }
    let offset = file_offset_for_rva(directory.rva, 24, optional.size_of_headers, sections)?;
    let tls = TlsDirectory32 {
        start_address_of_raw_data: read_u32(bytes, offset)?,
        end_address_of_raw_data: read_u32(bytes, checked_add(offset, 4)?)?,
        address_of_index: read_u32(bytes, checked_add(offset, 8)?)?,
        address_of_callbacks: read_u32(bytes, checked_add(offset, 12)?)?,
        size_of_zero_fill: read_u32(bytes, checked_add(offset, 16)?)?,
        characteristics: read_u32(bytes, checked_add(offset, 20)?)?,
    };
    if tls.end_address_of_raw_data < tls.start_address_of_raw_data {
        return Err(PeError::Invalid("TLS template end precedes its start"));
    }
    Ok(Some(tls))
}

fn parse_imports(
    bytes: &[u8],
    optional: &OptionalHeader32,
    sections: &[Section],
) -> Result<Vec<Import>, PeError> {
    let Some(directory) = optional
        .data_directories
        .get(IMPORT_DIRECTORY_INDEX)
        .copied()
    else {
        return Ok(Vec::new());
    };
    if directory.is_empty() {
        return Ok(Vec::new());
    }

    let mut imports = Vec::new();
    let mut descriptor_offset = 0_u32;
    while directory.size.saturating_sub(descriptor_offset) >= IMPORT_DESCRIPTOR_SIZE {
        let descriptor_rva = directory
            .rva
            .checked_add(descriptor_offset)
            .ok_or(PeError::Invalid("import descriptor RVA overflow"))?;
        let offset = file_offset_for_rva(
            descriptor_rva,
            IMPORT_DESCRIPTOR_SIZE,
            optional.size_of_headers,
            sections,
        )?;
        let original_first_thunk = read_u32(bytes, offset)?;
        let name_rva = read_u32(bytes, checked_add(offset, 12)?)?;
        let first_thunk = read_u32(bytes, checked_add(offset, 16)?)?;
        if original_first_thunk == 0 && name_rva == 0 && first_thunk == 0 {
            return Ok(imports);
        }
        if name_rva == 0 || first_thunk == 0 {
            return Err(PeError::Invalid("incomplete import descriptor"));
        }

        let module = read_rva_c_string(bytes, name_rva, optional.size_of_headers, sections)?;
        let lookup_table = if original_first_thunk == 0 {
            first_thunk
        } else {
            original_first_thunk
        };
        parse_import_thunks(
            bytes,
            optional.size_of_headers,
            sections,
            &module,
            lookup_table,
            first_thunk,
            &mut imports,
        )?;
        descriptor_offset = descriptor_offset
            .checked_add(IMPORT_DESCRIPTOR_SIZE)
            .ok_or(PeError::Invalid("import descriptor size overflow"))?;
    }
    Err(PeError::Invalid("unterminated import descriptor table"))
}

fn parse_import_thunks(
    bytes: &[u8],
    size_of_headers: u32,
    sections: &[Section],
    module: &str,
    lookup_table: u32,
    first_thunk: u32,
    imports: &mut Vec<Import>,
) -> Result<(), PeError> {
    let mut index = 0_u32;
    loop {
        let byte_offset = index
            .checked_mul(4)
            .ok_or(PeError::Invalid("import thunk index overflow"))?;
        let lookup_rva = lookup_table
            .checked_add(byte_offset)
            .ok_or(PeError::Invalid("import lookup RVA overflow"))?;
        let offset = file_offset_for_rva(lookup_rva, 4, size_of_headers, sections)?;
        let value = read_u32(bytes, offset)?;
        if value == 0 {
            return Ok(());
        }
        let iat_rva = first_thunk
            .checked_add(byte_offset)
            .ok_or(PeError::Invalid("IAT RVA overflow"))?;
        if value & ORDINAL_FLAG32 != 0 {
            imports.push(Import {
                module: module.to_owned(),
                name: None,
                ordinal: Some((value & 0xffff) as u16),
                iat_rva,
            });
        } else {
            let name_rva = value
                .checked_add(2)
                .ok_or(PeError::Invalid("import-by-name RVA overflow"))?;
            imports.push(Import {
                module: module.to_owned(),
                name: Some(read_rva_c_string(
                    bytes,
                    name_rva,
                    size_of_headers,
                    sections,
                )?),
                ordinal: None,
                iat_rva,
            });
        }
        index = index
            .checked_add(1)
            .ok_or(PeError::Invalid("import thunk count overflow"))?;
    }
}

fn parse_relocations(
    bytes: &[u8],
    optional: &OptionalHeader32,
    sections: &[Section],
) -> Result<Vec<Relocation>, PeError> {
    let Some(directory) = optional
        .data_directories
        .get(RELOCATION_DIRECTORY_INDEX)
        .copied()
    else {
        return Ok(Vec::new());
    };
    if directory.is_empty() {
        return Ok(Vec::new());
    }

    let mut relocations = Vec::new();
    let mut consumed = 0_u32;
    while consumed < directory.size {
        if directory.size - consumed < RELOCATION_BLOCK_HEADER_SIZE {
            return Err(PeError::Invalid("truncated relocation block header"));
        }
        let block_rva = directory
            .rva
            .checked_add(consumed)
            .ok_or(PeError::Invalid("relocation directory RVA overflow"))?;
        let header_offset = file_offset_for_rva(
            block_rva,
            RELOCATION_BLOCK_HEADER_SIZE,
            optional.size_of_headers,
            sections,
        )?;
        let page_rva = read_u32(bytes, header_offset)?;
        let block_size = read_u32(bytes, checked_add(header_offset, 4)?)?;
        if block_size < RELOCATION_BLOCK_HEADER_SIZE
            || block_size % 2 != 0
            || block_size > directory.size - consumed
        {
            return Err(PeError::Invalid("invalid relocation block size"));
        }

        let entry_bytes = block_size - RELOCATION_BLOCK_HEADER_SIZE;
        for entry_index in 0..entry_bytes / 2 {
            let entry_rva = block_rva
                .checked_add(RELOCATION_BLOCK_HEADER_SIZE)
                .and_then(|value| value.checked_add(entry_index * 2))
                .ok_or(PeError::Invalid("relocation entry RVA overflow"))?;
            let entry_offset =
                file_offset_for_rva(entry_rva, 2, optional.size_of_headers, sections)?;
            let entry = read_u16(bytes, entry_offset)?;
            let kind = (entry >> 12) as u8;
            if kind != 0 {
                relocations.push(Relocation {
                    rva: page_rva
                        .checked_add(u32::from(entry & 0x0fff))
                        .ok_or(PeError::Invalid("relocation target RVA overflow"))?,
                    kind,
                });
            }
        }
        consumed = consumed
            .checked_add(block_size)
            .ok_or(PeError::Invalid("relocation directory size overflow"))?;
    }
    Ok(relocations)
}

fn read_rva_c_string(
    bytes: &[u8],
    rva: u32,
    size_of_headers: u32,
    sections: &[Section],
) -> Result<String, PeError> {
    let (offset, available) = file_span_for_rva(rva, size_of_headers, sections)?;
    let span = bytes
        .get(offset..offset.saturating_add(available))
        .ok_or(PeError::Truncated { offset })?;
    let length = span
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(PeError::Invalid("unterminated PE string"))?;
    Ok(String::from_utf8_lossy(&span[..length]).into_owned())
}

fn file_offset_for_rva(
    rva: u32,
    size: u32,
    size_of_headers: u32,
    sections: &[Section],
) -> Result<usize, PeError> {
    let (offset, available) = file_span_for_rva(rva, size_of_headers, sections)?;
    if size > u32::try_from(available).unwrap_or(u32::MAX) {
        return Err(PeError::UnmappedRva { rva, size });
    }
    Ok(offset)
}

fn file_span_for_rva(
    rva: u32,
    size_of_headers: u32,
    sections: &[Section],
) -> Result<(usize, usize), PeError> {
    if rva < size_of_headers {
        return Ok((
            usize::try_from(rva).map_err(|_| PeError::UnmappedRva { rva, size: 1 })?,
            usize::try_from(size_of_headers - rva)
                .map_err(|_| PeError::UnmappedRva { rva, size: 1 })?,
        ));
    }
    for section in sections {
        let raw_end = section
            .virtual_address
            .checked_add(section.size_of_raw_data)
            .ok_or(PeError::Invalid("section RVA range overflow"))?;
        if rva >= section.virtual_address && rva < raw_end {
            let delta = rva - section.virtual_address;
            let file_offset = section
                .pointer_to_raw_data
                .checked_add(delta)
                .ok_or(PeError::Invalid("section file offset overflow"))?;
            return Ok((
                usize::try_from(file_offset).map_err(|_| PeError::UnmappedRva { rva, size: 1 })?,
                usize::try_from(section.size_of_raw_data - delta)
                    .map_err(|_| PeError::UnmappedRva { rva, size: 1 })?,
            ));
        }
    }
    Err(PeError::UnmappedRva { rva, size: 1 })
}

fn checked_add(base: usize, amount: usize) -> Result<usize, PeError> {
    base.checked_add(amount)
        .ok_or(PeError::Invalid("file offset overflow"))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, PeError> {
    if bytes.len().saturating_sub(offset) < size_of::<u16>() {
        return Err(PeError::Truncated { offset });
    }
    Ok(bytes.pread_with(offset, LE)?)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, PeError> {
    if bytes.len().saturating_sub(offset) < size_of::<u32>() {
        return Err(PeError::Truncated { offset });
    }
    Ok(bytes.pread_with(offset, LE)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_pe_input() {
        let error = parse(&[0; 128]).expect_err("input is not an MZ executable");
        assert!(matches!(error, PeError::Invalid("missing MZ signature")));
    }

    #[test]
    fn parses_minimal_pe32_headers() {
        let image = minimal_image();
        let parsed = parse(&image).expect("minimal image should parse");
        assert_eq!(parsed.entry_point(), 0x0040_1000);
        assert_eq!(parsed.sections[0].name, ".idata");
        assert_eq!(parsed.file_offset_for_rva(0x1000, 4).unwrap(), 0x200);
    }

    #[test]
    fn parses_named_and_ordinal_imports() {
        let mut image = minimal_image();
        set_directory(&mut image, IMPORT_DIRECTORY_INDEX, 0x1000, 40);
        put_u32(&mut image, 0x200, 0x1040);
        put_u32(&mut image, 0x20c, 0x1060);
        put_u32(&mut image, 0x210, 0x1050);
        put_u32(&mut image, 0x240, 0x1070);
        put_u32(&mut image, 0x244, ORDINAL_FLAG32 | 7);
        image[0x260..0x26b].copy_from_slice(b"USER32.dll\0");
        image[0x270..0x27e].copy_from_slice(b"\0\0MessageBoxA\0");

        let parsed = parse(&image).expect("imports should parse");
        assert_eq!(parsed.imports.len(), 2);
        assert_eq!(parsed.imports[0].module, "USER32.dll");
        assert_eq!(parsed.imports[0].name.as_deref(), Some("MessageBoxA"));
        assert_eq!(parsed.imports[0].iat_rva, 0x1050);
        assert_eq!(parsed.imports[1].ordinal, Some(7));
    }

    #[test]
    fn parses_highlow_relocation() {
        let mut image = minimal_image();
        set_directory(&mut image, RELOCATION_DIRECTORY_INDEX, 0x1080, 12);
        put_u32(&mut image, 0x280, 0x2000);
        put_u32(&mut image, 0x284, 12);
        image[0x288..0x28a].copy_from_slice(&0x3004_u16.to_le_bytes());

        let parsed = parse(&image).expect("relocations should parse");
        assert_eq!(
            parsed.relocations,
            vec![Relocation {
                rva: 0x2004,
                kind: 3,
            }]
        );
    }

    #[test]
    fn parses_pe32_tls_directory() {
        let mut image = minimal_image();
        set_directory(&mut image, TLS_DIRECTORY_INDEX, 0x10a0, 24);
        put_u32(&mut image, 0x2a0, 0x0040_1100);
        put_u32(&mut image, 0x2a4, 0x0040_1104);
        put_u32(&mut image, 0x2a8, 0x0040_1120);
        put_u32(&mut image, 0x2ac, 0x0040_1130);
        put_u32(&mut image, 0x2b0, 8);
        put_u32(&mut image, 0x2b4, 0x0030_0000);

        let parsed = parse(&image).expect("TLS directory should parse");
        assert_eq!(
            parsed.tls,
            Some(TlsDirectory32 {
                start_address_of_raw_data: 0x0040_1100,
                end_address_of_raw_data: 0x0040_1104,
                address_of_index: 0x0040_1120,
                address_of_callbacks: 0x0040_1130,
                size_of_zero_fill: 8,
                characteristics: 0x0030_0000,
            })
        );
    }

    fn minimal_image() -> Vec<u8> {
        let mut image = vec![0_u8; 0x400];
        image[0..2].copy_from_slice(&DOS_MAGIC.to_le_bytes());
        put_u32(&mut image, 0x3c, 0x80);
        put_u32(&mut image, 0x80, PE_SIGNATURE);
        image[0x84..0x86].copy_from_slice(&I386_MACHINE.to_le_bytes());
        image[0x86..0x88].copy_from_slice(&1_u16.to_le_bytes());
        image[0x94..0x96].copy_from_slice(&0xe0_u16.to_le_bytes());
        image[0x98..0x9a].copy_from_slice(&PE32_MAGIC.to_le_bytes());
        put_u32(&mut image, 0xa8, 0x1000);
        put_u32(&mut image, 0xb4, 0x0040_0000);
        put_u32(&mut image, 0xb8, 0x1000);
        put_u32(&mut image, 0xbc, 0x200);
        put_u32(&mut image, 0xd0, 0x2000);
        put_u32(&mut image, 0xd4, 0x200);
        put_u32(&mut image, 0xf4, 16);
        image[0x178..0x17e].copy_from_slice(b".idata");
        put_u32(&mut image, 0x180, 0x200);
        put_u32(&mut image, 0x184, 0x1000);
        put_u32(&mut image, 0x188, 0x200);
        put_u32(&mut image, 0x18c, 0x200);
        put_u32(&mut image, 0x19c, 0xc000_0040);
        image
    }

    fn set_directory(image: &mut [u8], index: usize, rva: u32, size: u32) {
        let offset = 0xf8 + index * DATA_DIRECTORY_SIZE;
        put_u32(image, offset, rva);
        put_u32(image, offset + 4, size);
    }

    fn put_u32(image: &mut [u8], offset: usize, value: u32) {
        image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
