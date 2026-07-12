use super::*;

pub(super) fn initialize_host_thunk_region(memory: &mut GuestMemory) -> Result<(), RuntimeError> {
    memory.map_range(
        GuestAddress(HOST_THUNK_BASE),
        HOST_THUNK_REGION_SIZE,
        Permissions::READ_WRITE,
    )?;
    let mut bytes = vec![0_u8; HOST_THUNK_REGION_SIZE as usize];
    for stub in bytes.chunks_exact_mut(4) {
        // A tiny readable x86 facade for software that inspects API entry
        // bytes. Execution is still intercepted by ThunkResolver before fetch.
        stub.copy_from_slice(&[0xc3, 0x90, 0x90, 0x90]); // RET; NOP x3
    }
    memory.write(GuestAddress(HOST_THUNK_BASE), &bytes)?;
    memory.protect_range(
        GuestAddress(HOST_THUNK_BASE),
        HOST_THUNK_REGION_SIZE,
        Permissions::READ_EXECUTE,
    )?;
    Ok(())
}

/// Install the small subset of 32-bit TEB/PEB fields used by process startup.
/// Unmodeled pointers remain zero so unsupported consumers fail predictably.
pub(super) fn initialize_win32_process_structures(
    memory: &mut GuestMemory,
    image_base: GuestAddress,
    process_parameters: GuestAddress,
    loader_data: GuestAddress,
    tls_slots: GuestAddress,
) -> Result<(), RuntimeError> {
    memory.map_range(
        GuestAddress(GUEST_TEB_BASE),
        PAGE_SIZE_U32,
        Permissions::READ_WRITE,
    )?;
    memory.map_range(
        GuestAddress(GUEST_PEB_BASE),
        PAGE_SIZE_U32,
        Permissions::READ_WRITE,
    )?;

    // NT_TIB / TEB: exception list, stack bounds, self pointer, client IDs,
    // PEB link, and the directly addressable LastError slot.
    memory.write_u32(GuestAddress(GUEST_TEB_BASE), u32::MAX)?;
    memory.write_u32(GuestAddress(GUEST_TEB_BASE + 0x04), GUEST_STACK_TOP)?;
    memory.write_u32(GuestAddress(GUEST_TEB_BASE + 0x08), GUEST_STACK_BASE)?;
    memory.write_u32(GuestAddress(GUEST_TEB_BASE + 0x18), GUEST_TEB_BASE)?;
    memory.write_u32(GuestAddress(GUEST_TEB_BASE + 0x20), GUEST_PROCESS_ID)?;
    memory.write_u32(GuestAddress(GUEST_TEB_BASE + 0x24), GUEST_THREAD_ID)?;
    memory.write_u32(GuestAddress(GUEST_TEB_BASE + 0x2c), tls_slots.0)?;
    memory.write_u32(GuestAddress(GUEST_TEB_BASE + 0x30), GUEST_PEB_BASE)?;
    memory.write_u32(GuestAddress(GUEST_TEB_BASE + TEB_LAST_ERROR_OFFSET), 0)?;

    // PEB core pointers reference runtime-owned normalized process state.
    memory.write_u32(GuestAddress(GUEST_PEB_BASE + 0x08), image_base.0)?;
    memory.write_u32(GuestAddress(GUEST_PEB_BASE + 0x0c), loader_data.0)?;
    memory.write_u32(GuestAddress(GUEST_PEB_BASE + 0x10), process_parameters.0)?;
    memory.write_u32(GuestAddress(GUEST_PEB_BASE + 0x18), PROCESS_HEAP_HANDLE)?;
    Ok(())
}

pub(super) fn initialize_host_module_image(
    memory: &mut GuestMemory,
    registry: &ApiRegistry,
    import_thunks: &mut HashMap<GuestAddress, ApiKey>,
    module: &str,
    base: GuestAddress,
) -> Result<(), RuntimeError> {
    let module_exports = registry
        .registered_keys()
        .into_iter()
        .filter(|key| key.module == module)
        .collect::<Vec<_>>();
    let mut named_exports = module_exports
        .iter()
        .filter(|key| !key.name.starts_with('#'))
        .cloned()
        .collect::<Vec<_>>();
    named_exports.sort_by(|left, right| left.name.cmp(&right.name));

    // IMAGE_EXPORT_DIRECTORY indexes functions relative to Base (1 here).
    // Preserve explicit ordinal identities such as COMCTL32!#17, then place
    // named exports into the remaining slots. Holes are valid PE entries and
    // remain zero, just as they do in native DLL export tables.
    let mut function_slots = Vec::<Option<ApiKey>>::new();
    for key in module_exports
        .iter()
        .filter(|key| key.name.starts_with('#'))
    {
        let ordinal = key.name[1..]
            .parse::<u16>()
            .map_err(|_| RuntimeError::Unsupported("invalid synthetic export ordinal"))?;
        if ordinal == 0 {
            return Err(RuntimeError::Unsupported("zero synthetic export ordinal"));
        }
        let index = usize::from(ordinal - 1);
        if function_slots.len() <= index {
            function_slots.resize(index + 1, None);
        }
        if function_slots[index].replace(key.clone()).is_some() {
            return Err(RuntimeError::Unsupported(
                "duplicate synthetic export ordinal",
            ));
        }
    }

    let mut named_slots = Vec::with_capacity(named_exports.len());
    for key in named_exports {
        let index = function_slots
            .iter()
            .position(Option::is_none)
            .unwrap_or(function_slots.len());
        if index == function_slots.len() {
            function_slots.push(None);
        }
        function_slots[index] = Some(key.clone());
        named_slots.push((key, index));
    }

    memory.map_range(base, HOST_MODULE_IMAGE_SIZE, Permissions::READ_WRITE)?;
    memory.write(base, b"MZ")?;
    memory.write_u32(GuestAddress(base.0 + 0x3c), 0x80)?;
    memory.write(GuestAddress(base.0 + 0x80), b"PE\0\0")?;
    memory.write_u16(GuestAddress(base.0 + 0x84), 0x014c)?;
    memory.write_u16(GuestAddress(base.0 + 0x94), 0x00e0)?;
    memory.write_u16(GuestAddress(base.0 + 0x98), 0x010b)?;

    let export_directory_rva = 0x200_u32;
    let export_directory = GuestAddress(base.0 + export_directory_rva);
    let function_count = u32::try_from(function_slots.len())
        .map_err(|_| RuntimeError::Unsupported("too many synthetic module exports"))?;
    let name_count = u32::try_from(named_slots.len())
        .map_err(|_| RuntimeError::Unsupported("too many synthetic named exports"))?;
    let functions_rva = 0x228_u32;
    let names_rva = functions_rva
        .checked_add(function_count.saturating_mul(4))
        .ok_or(RuntimeError::Unsupported("synthetic export table overflow"))?;
    let ordinals_rva = names_rva
        .checked_add(name_count.saturating_mul(4))
        .ok_or(RuntimeError::Unsupported("synthetic export table overflow"))?;
    let mut string_rva = align_up(
        ordinals_rva
            .checked_add(name_count.saturating_mul(2))
            .ok_or(RuntimeError::Unsupported("synthetic export table overflow"))?,
        4,
    )
    .ok_or(RuntimeError::Unsupported("synthetic export table overflow"))?;

    let module_name_rva = string_rva;
    let mut module_name = module.as_bytes().to_vec();
    module_name.push(0);
    memory.write(GuestAddress(base.0 + string_rva), &module_name)?;
    string_rva = string_rva
        .checked_add(u32::try_from(module_name.len()).unwrap_or(u32::MAX))
        .ok_or(RuntimeError::Unsupported(
            "synthetic export strings overflow",
        ))?;

    for (index, key) in function_slots.iter().enumerate() {
        let Some(key) = key else {
            continue;
        };
        let thunk = if let Some((address, _)) =
            import_thunks.iter().find(|(_, existing)| *existing == key)
        {
            *address
        } else {
            let address = next_free_host_thunk(import_thunks)
                .ok_or(RuntimeError::Unsupported("Host thunk table overflow"))?;
            import_thunks.insert(address, key.clone());
            address
        };
        let index = u32::try_from(index)
            .map_err(|_| RuntimeError::Unsupported("synthetic export index overflow"))?;
        memory.write_u32(
            GuestAddress(base.0 + functions_rva + index * 4),
            thunk.0.wrapping_sub(base.0),
        )?;
    }

    for (name_index, (key, function_index)) in named_slots.iter().enumerate() {
        let name_index = u32::try_from(name_index)
            .map_err(|_| RuntimeError::Unsupported("synthetic export index overflow"))?;
        let function_index = u16::try_from(*function_index)
            .map_err(|_| RuntimeError::Unsupported("synthetic export ordinal overflow"))?;
        memory.write_u32(
            GuestAddress(base.0 + names_rva + name_index * 4),
            string_rva,
        )?;
        memory.write_u16(
            GuestAddress(base.0 + ordinals_rva + name_index * 2),
            function_index,
        )?;
        let mut name = key.name.as_bytes().to_vec();
        name.push(0);
        memory.write(GuestAddress(base.0 + string_rva), &name)?;
        string_rva = string_rva
            .checked_add(u32::try_from(name.len()).unwrap_or(u32::MAX))
            .filter(|end| *end < HOST_MODULE_IMAGE_SIZE)
            .ok_or(RuntimeError::Unsupported(
                "synthetic export strings overflow",
            ))?;
    }

    memory.write_u32(GuestAddress(export_directory.0 + 12), module_name_rva)?;
    memory.write_u32(GuestAddress(export_directory.0 + 16), 1)?;
    memory.write_u32(GuestAddress(export_directory.0 + 20), function_count)?;
    memory.write_u32(GuestAddress(export_directory.0 + 24), name_count)?;
    memory.write_u32(GuestAddress(export_directory.0 + 28), functions_rva)?;
    memory.write_u32(GuestAddress(export_directory.0 + 32), names_rva)?;
    memory.write_u32(GuestAddress(export_directory.0 + 36), ordinals_rva)?;
    memory.write_u32(GuestAddress(base.0 + 0x80 + 0x78), export_directory_rva)?;
    memory.write_u32(
        GuestAddress(base.0 + 0x80 + 0x7c),
        string_rva - export_directory_rva,
    )?;
    memory.protect_range(base, HOST_MODULE_IMAGE_SIZE, Permissions::READ)?;
    Ok(())
}

/// Find an unused Host thunk slot without assuming the address map is dense.
///
/// Dynamic export resolution and explicit thunk registration can create holes.
/// Using `HashMap::len()` as an address index can then overwrite a live slot and
/// silently change the ABI/stack cleanup of an already-bound Guest IAT entry.
pub(super) fn next_free_host_thunk(
    import_thunks: &HashMap<GuestAddress, ApiKey>,
) -> Option<GuestAddress> {
    let slot_count = HOST_THUNK_REGION_SIZE / 4;
    (0..slot_count).find_map(|index| {
        let address = HOST_THUNK_BASE.checked_add(index.checked_mul(4)?)?;
        let address = GuestAddress(address);
        (!import_thunks.contains_key(&address)).then_some(address)
    })
}

pub(super) fn initialize_host_modules(
    memory: &mut GuestMemory,
    registry: &ApiRegistry,
    import_thunks: &mut HashMap<GuestAddress, ApiKey>,
) -> Result<HashMap<String, GuestAddress>, RuntimeError> {
    let mut modules = registry
        .registered_keys()
        .into_iter()
        .map(|key| key.module)
        .collect::<Vec<_>>();
    modules.extend([
        "kernel32.dll".to_owned(),
        "user32.dll".to_owned(),
        "ntdll.dll".to_owned(),
    ]);
    modules.sort();
    modules.dedup();

    let mut host_modules = HashMap::new();
    host_modules.insert(
        "kernel32.dll".to_owned(),
        GuestAddress(KERNEL32_MODULE_HANDLE),
    );
    host_modules.insert("user32.dll".to_owned(), GuestAddress(USER32_MODULE_HANDLE));
    host_modules.insert("ntdll.dll".to_owned(), GuestAddress(NTDLL_MODULE_HANDLE));
    let mut next_handle = NTDLL_MODULE_HANDLE + HOST_MODULE_IMAGE_SIZE;
    for module in modules {
        let handle = if let Some(handle) = host_modules.get(&module).copied() {
            handle
        } else {
            if next_handle >= GUEST_STACK_BASE {
                return Err(RuntimeError::Unsupported("too many synthetic Host DLLs"));
            }
            let handle = GuestAddress(next_handle);
            host_modules.insert(module.clone(), handle);
            next_handle += HOST_MODULE_IMAGE_SIZE;
            handle
        };
        initialize_host_module_image(memory, registry, import_thunks, &module, handle)?;
    }
    Ok(host_modules)
}

pub(super) struct StaticTlsLayout {
    pub(super) slots: GuestAddress,
    pub(super) callbacks: Vec<GuestAddress>,
}

pub(super) fn initialize_static_tls(
    memory: &mut GuestMemory,
    image: &PeImage,
) -> Result<StaticTlsLayout, RuntimeError> {
    let slots = GuestAddress(GUEST_TLS_BASE);
    memory.map_range(slots, GUEST_TLS_SIZE, Permissions::READ_WRITE)?;
    let Some(directory) = image.tls else {
        return Ok(StaticTlsLayout {
            slots,
            callbacks: Vec::new(),
        });
    };
    if directory.address_of_index == 0 {
        return Err(RuntimeError::Unsupported(
            "PE TLS directory has no index address",
        ));
    }

    let template_size = directory
        .end_address_of_raw_data
        .checked_sub(directory.start_address_of_raw_data)
        .ok_or(RuntimeError::Unsupported("PE TLS template range underflow"))?;
    let allocation_size = template_size
        .checked_add(directory.size_of_zero_fill)
        .ok_or(RuntimeError::Unsupported("PE TLS allocation size overflow"))?;
    if allocation_size > GUEST_TLS_SIZE - PAGE_SIZE_U32 {
        return Err(RuntimeError::Unsupported(
            "PE TLS allocation exceeds the reserved TLS region",
        ));
    }
    if template_size != 0 {
        let mut template = vec![
            0;
            usize::try_from(template_size).map_err(|_| {
                RuntimeError::Unsupported("PE TLS template does not fit host usize")
            })?
        ];
        memory.read(
            GuestAddress(directory.start_address_of_raw_data),
            &mut template,
        )?;
        memory.write(GuestAddress(GUEST_TLS_DATA_BASE), &template)?;
    }
    memory.write_u32(slots, GUEST_TLS_DATA_BASE)?;
    memory.write_u32(GuestAddress(directory.address_of_index), 0)?;

    let callbacks = read_tls_callbacks(memory, directory.address_of_callbacks)?;
    Ok(StaticTlsLayout { slots, callbacks })
}

pub(super) fn read_tls_callbacks(
    memory: &GuestMemory,
    callback_array: u32,
) -> Result<Vec<GuestAddress>, RuntimeError> {
    if callback_array == 0 {
        return Ok(Vec::new());
    }
    let mut callbacks = Vec::new();
    for index in 0..MAX_TLS_CALLBACKS {
        let byte_offset = u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or(RuntimeError::Unsupported("TLS callback index overflow"))?;
        let address = callback_array
            .checked_add(byte_offset)
            .ok_or(RuntimeError::Unsupported(
                "TLS callback array address overflow",
            ))?;
        let callback = memory.read_u32(GuestAddress(address))?;
        if callback == 0 {
            return Ok(callbacks);
        }
        callbacks.push(GuestAddress(callback));
    }
    Err(RuntimeError::Unsupported(
        "TLS callback array exceeds the configured safety limit",
    ))
}

pub(super) fn enter_tls_callback(
    cpu: &mut Interpreter,
    memory: &mut GuestMemory,
    callback: GuestAddress,
    image_base: GuestAddress,
) -> Result<(), RuntimeError> {
    let frame = cpu
        .state
        .registers
        .esp
        .checked_sub(16)
        .ok_or(RuntimeError::Unsupported("TLS callback stack underflow"))?;
    memory.write_u32(GuestAddress(frame), TLS_CALLBACK_RETURN_ADDRESS)?;
    memory.write_u32(GuestAddress(frame + 4), image_base.0)?;
    memory.write_u32(GuestAddress(frame + 8), 1)?; // DLL_PROCESS_ATTACH
    memory.write_u32(GuestAddress(frame + 12), 0)?;
    cpu.state.registers.esp = frame;
    cpu.state.registers.eip = callback.0;
    Ok(())
}

/// Enter the PE process entry point through the same termination boundary the
/// native Win32 startup wrapper provides. A process entry point is a normal
/// function: returning its value must terminate the initial thread/process,
/// not fetch an uninitialized return address from the top of the Guest stack.
pub(super) fn enter_main_entry_point(
    cpu: &mut Interpreter,
    memory: &mut GuestMemory,
    entry_point: GuestAddress,
) -> Result<(), RuntimeError> {
    let return_slot = GUEST_STACK_TOP
        .checked_sub(4)
        .ok_or(RuntimeError::Unsupported("process entry stack underflow"))?;
    memory.write_u32(GuestAddress(return_slot), THREAD_EXIT_RETURN_ADDRESS)?;
    cpu.state.registers.esp = return_slot;
    cpu.state.registers.eip = entry_point.0;
    Ok(())
}

pub(super) fn enter_stdcall_callback(
    cpu: &mut Interpreter,
    memory: &mut GuestMemory,
    callback: GuestAddress,
    arguments: &[u32],
    stack_pointer: u32,
) -> Result<(), RuntimeError> {
    let argument_bytes = u32::try_from(arguments.len())
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or(RuntimeError::Unsupported(
            "Guest callback argument size overflow",
        ))?;
    let frame_size = argument_bytes
        .checked_add(4)
        .ok_or(RuntimeError::Unsupported("Guest callback frame overflow"))?;
    let frame = stack_pointer
        .checked_sub(frame_size)
        .ok_or(RuntimeError::Unsupported("Guest callback stack underflow"))?;
    memory.write_u32(GuestAddress(frame), HOST_CALLBACK_RETURN_ADDRESS)?;
    for (index, argument) in arguments.iter().enumerate() {
        let offset = u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .and_then(|value| value.checked_add(4))
            .ok_or(RuntimeError::Unsupported(
                "Guest callback argument offset overflow",
            ))?;
        memory.write_u32(GuestAddress(frame + offset), *argument)?;
    }
    cpu.state.registers.esp = frame;
    cpu.state.registers.eip = callback.0;
    Ok(())
}

pub(super) struct ProcessDataLayout {
    pub(super) command_line_ansi: GuestAddress,
    pub(super) command_line_utf16: GuestAddress,
    pub(super) process_parameters: GuestAddress,
    pub(super) environment_ansi: GuestAddress,
    pub(super) environment_utf16: GuestAddress,
    pub(super) loader_data: GuestAddress,
}

pub(super) fn initialize_process_data(
    memory: &mut GuestMemory,
    config: &RuntimeConfig,
    image_base: GuestAddress,
    entry_point: GuestAddress,
    image_size: u32,
) -> Result<ProcessDataLayout, RuntimeError> {
    let base = GuestAddress(GUEST_PROCESS_DATA_BASE);
    memory.map_range(base, GUEST_PROCESS_DATA_SIZE, Permissions::READ_WRITE)?;

    let mut cursor = GUEST_PROCESS_DATA_BASE;
    let command_line_ansi = write_process_bytes(
        memory,
        &mut cursor,
        &vnrt_win32::encode_ansi_z(&config.command_line),
    )?;
    cursor = align_up(cursor, 2).ok_or(RuntimeError::Unsupported(
        "process string alignment overflow",
    ))?;
    let command_line_utf16_bytes = vnrt_win32::encode_utf16_z(&config.command_line);
    let command_line_utf16 = write_process_bytes(memory, &mut cursor, &command_line_utf16_bytes)?;
    let image_path_bytes = vnrt_win32::encode_utf16_z(&config.module_path);
    let image_path = write_process_bytes(memory, &mut cursor, &image_path_bytes)?;
    let current_directory_bytes = vnrt_win32::encode_utf16_z(&config.current_directory);
    let current_directory = write_process_bytes(memory, &mut cursor, &current_directory_bytes)?;
    let environment_ansi_bytes = encode_environment_block_ansi(&config.environment);
    let environment_ansi = write_process_bytes(memory, &mut cursor, &environment_ansi_bytes)?;
    cursor = align_up(cursor, 2).ok_or(RuntimeError::Unsupported(
        "environment block alignment overflow",
    ))?;
    let environment_utf16_bytes = encode_environment_block_utf16(&config.environment);
    let environment_utf16 = write_process_bytes(memory, &mut cursor, &environment_utf16_bytes)?;
    let loader_data = initialize_loader_data(
        memory,
        &mut cursor,
        config,
        image_base,
        entry_point,
        image_size,
    )?;

    let parameters = GuestAddress(GUEST_PROCESS_PARAMETERS_BASE);
    memory.write_u32(parameters, GUEST_PROCESS_PARAMETERS_SIZE)?;
    memory.write_u32(GuestAddress(parameters.0 + 0x04), 0x290)?;
    memory.write_u32(GuestAddress(parameters.0 + 0x08), 1)?; // normalized pointers
    memory.write_u32(GuestAddress(parameters.0 + 0x18), STD_INPUT_HANDLE_VALUE)?;
    memory.write_u32(GuestAddress(parameters.0 + 0x1c), STD_OUTPUT_HANDLE_VALUE)?;
    memory.write_u32(GuestAddress(parameters.0 + 0x20), STD_ERROR_HANDLE_VALUE)?;
    write_unicode_string(
        memory,
        GuestAddress(parameters.0 + 0x24),
        current_directory,
        &current_directory_bytes,
    )?;
    write_unicode_string(
        memory,
        GuestAddress(parameters.0 + 0x38),
        image_path,
        &image_path_bytes,
    )?;
    write_unicode_string(
        memory,
        GuestAddress(parameters.0 + 0x40),
        command_line_utf16,
        &command_line_utf16_bytes,
    )?;
    memory.write_u32(GuestAddress(parameters.0 + 0x48), environment_utf16.0)?;

    Ok(ProcessDataLayout {
        command_line_ansi,
        command_line_utf16,
        process_parameters: parameters,
        environment_ansi,
        environment_utf16,
        loader_data,
    })
}

struct LoaderModuleRecord {
    dll_base: GuestAddress,
    entry_point: GuestAddress,
    image_size: u32,
    full_name: GuestAddress,
    full_name_bytes: Vec<u8>,
    base_name: GuestAddress,
    base_name_bytes: Vec<u8>,
}

pub(super) fn initialize_loader_data(
    memory: &mut GuestMemory,
    cursor: &mut u32,
    config: &RuntimeConfig,
    image_base: GuestAddress,
    entry_point: GuestAddress,
    image_size: u32,
) -> Result<GuestAddress, RuntimeError> {
    let main_base_name = config
        .module_path
        .rsplit(['\\', '/'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(config.module_path.as_str());
    let modules = [
        (
            image_base,
            entry_point,
            image_size,
            config.module_path.as_str(),
            main_base_name,
        ),
        (
            GuestAddress(KERNEL32_MODULE_HANDLE),
            GuestAddress(0),
            0,
            r"C:\Windows\System32\kernel32.dll",
            "kernel32.dll",
        ),
        (
            GuestAddress(USER32_MODULE_HANDLE),
            GuestAddress(0),
            0,
            r"C:\Windows\System32\user32.dll",
            "user32.dll",
        ),
    ];
    let mut records = Vec::with_capacity(modules.len());
    for (dll_base, module_entry, module_size, full_name, base_name) in modules {
        let full_name_bytes = vnrt_win32::encode_utf16_z(full_name);
        let full_name = write_process_bytes(memory, cursor, &full_name_bytes)?;
        let base_name_bytes = vnrt_win32::encode_utf16_z(base_name);
        let base_name = write_process_bytes(memory, cursor, &base_name_bytes)?;
        records.push(LoaderModuleRecord {
            dll_base,
            entry_point: module_entry,
            image_size: module_size,
            full_name,
            full_name_bytes,
            base_name,
            base_name_bytes,
        });
    }

    let loader_data = reserve_process_data(cursor, 0x30, 4)?;
    let entries = records
        .iter()
        .map(|_| reserve_process_data(cursor, 0x80, 4))
        .collect::<Result<Vec<_>, _>>()?;
    memory.write_u32(loader_data, 0x30)?;
    memory.write_u8(GuestAddress(loader_data.0 + 0x04), 1)?;

    for (entry, record) in entries.iter().zip(&records) {
        memory.write_u32(GuestAddress(entry.0 + 0x18), record.dll_base.0)?;
        memory.write_u32(GuestAddress(entry.0 + 0x1c), record.entry_point.0)?;
        memory.write_u32(GuestAddress(entry.0 + 0x20), record.image_size)?;
        write_unicode_string(
            memory,
            GuestAddress(entry.0 + 0x24),
            record.full_name,
            &record.full_name_bytes,
        )?;
        write_unicode_string(
            memory,
            GuestAddress(entry.0 + 0x2c),
            record.base_name,
            &record.base_name_bytes,
        )?;
        memory.write_u16(GuestAddress(entry.0 + 0x38), 1)?;
    }

    for (head_offset, link_offset) in [(0x0c, 0), (0x14, 8), (0x1c, 0x10)] {
        let head = GuestAddress(loader_data.0 + head_offset);
        let links = entries
            .iter()
            .map(|entry| GuestAddress(entry.0 + link_offset))
            .collect::<Vec<_>>();
        write_circular_list(memory, head, &links)?;
    }
    Ok(loader_data)
}

pub(super) fn reserve_process_data(
    cursor: &mut u32,
    size: u32,
    alignment: u32,
) -> Result<GuestAddress, RuntimeError> {
    let start = align_up(*cursor, alignment).ok_or(RuntimeError::Unsupported(
        "process structure alignment overflow",
    ))?;
    let end = start
        .checked_add(size)
        .filter(|end| *end <= GUEST_PROCESS_PARAMETERS_BASE)
        .ok_or(RuntimeError::Unsupported(
            "process structures exceed the reserved region",
        ))?;
    *cursor = end;
    Ok(GuestAddress(start))
}

pub(super) fn write_circular_list(
    memory: &mut GuestMemory,
    head: GuestAddress,
    links: &[GuestAddress],
) -> Result<(), RuntimeError> {
    let Some(first) = links.first().copied() else {
        memory.write_u32(head, head.0)?;
        memory.write_u32(GuestAddress(head.0 + 4), head.0)?;
        return Ok(());
    };
    let last = links.last().copied().unwrap_or(first);
    memory.write_u32(head, first.0)?;
    memory.write_u32(GuestAddress(head.0 + 4), last.0)?;
    for (index, link) in links.iter().enumerate() {
        let previous = index
            .checked_sub(1)
            .and_then(|previous| links.get(previous).copied())
            .unwrap_or(head);
        let next = links.get(index + 1).copied().unwrap_or(head);
        memory.write_u32(*link, next.0)?;
        memory.write_u32(GuestAddress(link.0 + 4), previous.0)?;
    }
    Ok(())
}

pub(super) fn write_process_bytes(
    memory: &mut GuestMemory,
    cursor: &mut u32,
    bytes: &[u8],
) -> Result<GuestAddress, RuntimeError> {
    let length = u32::try_from(bytes.len())
        .map_err(|_| RuntimeError::Unsupported("process data value too long"))?;
    let end = cursor
        .checked_add(length)
        .filter(|end| *end <= GUEST_PROCESS_PARAMETERS_BASE)
        .ok_or(RuntimeError::Unsupported(
            "process data exceeds the reserved region",
        ))?;
    let address = GuestAddress(*cursor);
    memory.write(address, bytes)?;
    *cursor = end;
    Ok(address)
}

pub(super) fn write_unicode_string(
    memory: &mut GuestMemory,
    descriptor: GuestAddress,
    buffer: GuestAddress,
    nul_terminated_bytes: &[u8],
) -> Result<(), RuntimeError> {
    let maximum_length = u16::try_from(nul_terminated_bytes.len())
        .map_err(|_| RuntimeError::Unsupported("process Unicode string too long"))?;
    let length = maximum_length
        .checked_sub(2)
        .ok_or(RuntimeError::Unsupported("invalid process Unicode string"))?;
    memory.write_u16(descriptor, length)?;
    memory.write_u16(GuestAddress(descriptor.0 + 2), maximum_length)?;
    memory.write_u32(GuestAddress(descriptor.0 + 4), buffer.0)?;
    Ok(())
}

pub(super) fn encode_environment_block_utf16(environment: &BTreeMap<String, String>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (name, value) in environment {
        bytes.extend(vnrt_win32::encode_utf16_z(&format!("{name}={value}")));
    }
    // Environment blocks end in an additional UTF-16 NUL. An empty block
    // still has two terminators.
    if environment.is_empty() {
        bytes.extend_from_slice(&[0, 0]);
    }
    bytes.extend_from_slice(&[0, 0]);
    bytes
}

pub(super) fn encode_environment_block_ansi(environment: &BTreeMap<String, String>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (name, value) in environment {
        bytes.extend(vnrt_win32::encode_ansi_z(&format!("{name}={value}")));
    }
    if environment.is_empty() {
        bytes.push(0);
    }
    bytes.push(0);
    bytes
}
