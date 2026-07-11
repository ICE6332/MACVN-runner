use super::*;

pub(super) fn map_image(
    memory: &mut GuestMemory,
    bytes: &[u8],
    image: &PeImage,
) -> Result<(), RuntimeError> {
    let base = GuestAddress(image.optional.image_base);
    let mapped_size = align_up(image.optional.size_of_image, PAGE_SIZE_U32)
        .ok_or(RuntimeError::Unsupported("image size overflow"))?;
    if mapped_size == 0 {
        return Err(RuntimeError::Unsupported("zero-sized PE image"));
    }
    memory.map_range(base, mapped_size, Permissions::ALL)?;

    let header_len = usize::try_from(image.optional.size_of_headers)
        .unwrap_or(usize::MAX)
        .min(bytes.len());
    memory.write(base, &bytes[..header_len])?;
    for section in &image.sections {
        copy_section(memory, bytes, image.optional.image_base, section)?;
    }
    Ok(())
}

pub(super) fn bind_imports(
    memory: &mut GuestMemory,
    image: &PeImage,
) -> Result<HashMap<GuestAddress, ApiKey>, RuntimeError> {
    let image_end = image
        .optional
        .image_base
        .checked_add(image.optional.size_of_image)
        .ok_or(RuntimeError::Unsupported("image address range overflow"))?;
    if image_end > HOST_THUNK_BASE {
        return Err(RuntimeError::Unsupported(
            "PE image overlaps the reserved host thunk region",
        ));
    }

    let mut thunks = HashMap::with_capacity(image.imports.len());
    for (index, import) in image.imports.iter().enumerate() {
        let index = u32::try_from(index)
            .map_err(|_| RuntimeError::Unsupported("too many imported APIs"))?;
        let thunk = HOST_THUNK_BASE
            .checked_add(
                index
                    .checked_mul(4)
                    .ok_or(RuntimeError::Unsupported("host thunk table overflow"))?,
            )
            .ok_or(RuntimeError::Unsupported("host thunk table overflow"))?;
        let key = import_api_key(import)?;
        let iat_address = image
            .optional
            .image_base
            .checked_add(import.iat_rva)
            .ok_or(RuntimeError::Unsupported("IAT address overflow"))?;
        memory.write_u32(GuestAddress(iat_address), thunk)?;
        thunks.insert(GuestAddress(thunk), key);
    }
    Ok(thunks)
}

pub(super) fn import_api_key(import: &Import) -> Result<ApiKey, RuntimeError> {
    let name = match (&import.name, import.ordinal) {
        (Some(name), None) => name.clone(),
        (None, Some(ordinal)) => format!("#{ordinal}"),
        _ => return Err(RuntimeError::Unsupported("invalid PE import identity")),
    };
    Ok(ApiKey::new(import.module.clone(), name))
}

pub(super) fn protect_image(memory: &mut GuestMemory, image: &PeImage) -> Result<(), RuntimeError> {
    let base = GuestAddress(image.optional.image_base);
    let mapped_size = align_up(image.optional.size_of_image, PAGE_SIZE_U32)
        .ok_or(RuntimeError::Unsupported("image size overflow"))?;

    memory.protect_range(base, mapped_size, Permissions::READ)?;
    for section in &image.sections {
        let section_start = image
            .optional
            .image_base
            .checked_add(section.virtual_address)
            .ok_or(RuntimeError::Unsupported("section address overflow"))?;
        let page_start = section_start & !(PAGE_SIZE_U32 - 1);
        let page_delta = section_start - page_start;
        let section_size = section.virtual_size.max(section.size_of_raw_data);
        let protected_size = align_up(
            page_delta
                .checked_add(section_size)
                .ok_or(RuntimeError::Unsupported("section size overflow"))?,
            PAGE_SIZE_U32,
        )
        .ok_or(RuntimeError::Unsupported(
            "section protection size overflow",
        ))?;
        if protected_size != 0 {
            memory.protect_range(
                GuestAddress(page_start),
                protected_size,
                section_permissions(section),
            )?;
        }
    }
    Ok(())
}

pub(super) fn copy_section(
    memory: &mut GuestMemory,
    bytes: &[u8],
    image_base: u32,
    section: &Section,
) -> Result<(), RuntimeError> {
    if section.size_of_raw_data == 0 {
        return Ok(());
    }
    let start = usize::try_from(section.pointer_to_raw_data).map_err(|_| {
        RuntimeError::TruncatedSection {
            name: section.name.clone(),
        }
    })?;
    let len =
        usize::try_from(section.size_of_raw_data).map_err(|_| RuntimeError::TruncatedSection {
            name: section.name.clone(),
        })?;
    let raw = bytes.get(start..start.saturating_add(len)).ok_or_else(|| {
        RuntimeError::TruncatedSection {
            name: section.name.clone(),
        }
    })?;
    let address = image_base
        .checked_add(section.virtual_address)
        .ok_or(RuntimeError::Unsupported("section address overflow"))?;
    memory.write(GuestAddress(address), raw)?;
    Ok(())
}

pub(super) fn section_permissions(section: &Section) -> Permissions {
    Permissions::new(
        section.characteristics & 0x4000_0000 != 0,
        section.characteristics & 0x8000_0000 != 0,
        section.characteristics & 0x2000_0000 != 0,
    )
}

pub(super) fn align_up(value: u32, alignment: u32) -> Option<u32> {
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
}
