use super::*;

#[test]
fn alignment_is_checked() {
    assert_eq!(align_up(1, 4096), Some(4096));
    assert_eq!(align_up(4096, 4096), Some(4096));
    assert_eq!(align_up(u32::MAX, 4096), None);
}

#[test]
fn empty_environment_blocks_have_double_terminators() {
    let environment = BTreeMap::new();
    assert_eq!(encode_environment_block_ansi(&environment), [0, 0]);
    assert_eq!(encode_environment_block_utf16(&environment), [0, 0, 0, 0]);
}

#[test]
fn loader_list_links_are_circular_and_bidirectional() {
    let mut memory = GuestMemory::new();
    memory
        .map_range(GuestAddress(0x1000), PAGE_SIZE_U32, Permissions::READ_WRITE)
        .unwrap();
    let head = GuestAddress(0x1000);
    let links = [
        GuestAddress(0x1100),
        GuestAddress(0x1200),
        GuestAddress(0x1300),
    ];

    write_circular_list(&mut memory, head, &links).unwrap();

    assert_eq!(memory.read_u32(head).unwrap(), links[0].0);
    assert_eq!(
        memory.read_u32(GuestAddress(head.0 + 4)).unwrap(),
        links[2].0
    );
    assert_eq!(memory.read_u32(links[0]).unwrap(), links[1].0);
    assert_eq!(
        memory.read_u32(GuestAddress(links[0].0 + 4)).unwrap(),
        head.0
    );
    assert_eq!(memory.read_u32(links[2]).unwrap(), head.0);
    assert_eq!(
        memory.read_u32(GuestAddress(links[2].0 + 4)).unwrap(),
        links[1].0
    );
}

#[test]
fn headless_audio_is_explicitly_unsupported() {
    let error = HeadlessBackend
        .submit_audio(&[])
        .expect_err("headless backend must not silently discard audio");
    assert!(matches!(error, RuntimeError::Unsupported(_)));
}

#[test]
fn private_heaps_preserve_reallocated_bytes_and_ownership() {
    let mut memory = GuestMemory::new();
    let mut heaps = GuestHeapManager::new();
    let heap = heaps.create(16, 64, false).unwrap();
    let allocation = heaps.allocate(&mut memory, heap, 8).unwrap();
    memory.write(allocation, &[0xaa, 0xbb, 0xcc]).unwrap();

    let replacement = heaps.reallocate(&mut memory, heap, allocation, 16).unwrap();
    let mut bytes = [0xff; 16];
    memory.read(replacement, &mut bytes).unwrap();
    assert_eq!(&bytes[..3], &[0xaa, 0xbb, 0xcc]);
    assert_eq!(&bytes[8..], &[0; 8]);
    assert_eq!(heaps.size(heap, replacement).unwrap(), 16);
    assert!(matches!(
        heaps.size(Handle(PROCESS_HEAP_HANDLE), replacement),
        Err(Win32Error::InvalidAllocation { .. })
    ));

    heaps.destroy(&mut memory, heap).unwrap();
    assert!(matches!(
        heaps.size(heap, replacement),
        Err(Win32Error::InvalidHandle(_))
    ));
    assert!(memory.read_u8(replacement).is_ok());
}

#[test]
fn executable_private_heap_allows_guest_instruction_fetch() {
    let mut memory = GuestMemory::new();
    let mut heaps = GuestHeapManager::new();
    let heap = heaps.create(0, 0, true).unwrap();
    let allocation = heaps.allocate(&mut memory, heap, 8).unwrap();
    memory.write(allocation, &[0xf4]).unwrap();

    let mut instruction = [0; 1];
    memory.fetch(allocation, &mut instruction).unwrap();
    assert_eq!(instruction, [0xf4]);
}

#[test]
fn loader_binds_named_imports_into_the_iat() {
    let image = image_with_one_import();
    let runtime = Runtime::load(&image, ApiRegistry::new()).expect("image should load");
    assert_eq!(
        runtime.memory.read_u32(GuestAddress(0x0040_1050)).unwrap(),
        HOST_THUNK_BASE
    );
    assert_eq!(
        runtime.import_thunks.get(&GuestAddress(HOST_THUNK_BASE)),
        Some(&ApiKey::new("USER32.dll", "MessageBoxA"))
    );
}

#[test]
fn synthetic_kernel32_image_exports_registered_host_thunks() {
    let image = image_with_one_import();
    let mut registry = ApiRegistry::new();
    vnrt_kernel32::register(&mut registry);
    let runtime = Runtime::load(&image, registry).expect("image should load");
    let base = GuestAddress(KERNEL32_MODULE_HANDLE);
    let nt = base.0
        + runtime
            .memory
            .read_u32(GuestAddress(base.0 + 0x3c))
            .unwrap();
    let export_rva = runtime.memory.read_u32(GuestAddress(nt + 0x78)).unwrap();
    let export = base.0 + export_rva;
    let count = runtime.memory.read_u32(GuestAddress(export + 24)).unwrap();
    let functions_rva = runtime.memory.read_u32(GuestAddress(export + 28)).unwrap();
    let first_function_rva = runtime
        .memory
        .read_u32(GuestAddress(base.0 + functions_rva))
        .unwrap();

    assert!(count > 0);
    assert_eq!(runtime.memory.read_u16(base).unwrap(), 0x5a4d);
    assert_eq!(
        runtime.memory.read_u32(GuestAddress(nt)).unwrap(),
        0x0000_4550
    );
    assert!(
        runtime
            .import_thunks
            .contains_key(&GuestAddress(base.0.wrapping_add(first_function_rva)))
    );
}

#[test]
fn ordinal_imports_have_stable_host_call_keys() {
    let import = Import {
        module: "COMCTL32.dll".to_owned(),
        name: None,
        ordinal: Some(17),
        iat_rva: 0x1234,
    };

    assert_eq!(
        import_api_key(&import).unwrap(),
        ApiKey::new("comctl32.dll", "#17")
    );
}

#[test]
fn runs_a_pe32_guest_until_exit_process() {
    let image = image_that_calls_exit_process(42);
    let mut registry = ApiRegistry::new();
    vnrt_kernel32::register(&mut registry);
    let mut runtime = Runtime::load(&image, registry).expect("guest should load");

    let outcome = runtime
        .run(RunLimits {
            max_instructions: 8,
        })
        .expect("guest should reach ExitProcess");

    assert_eq!(outcome, RunOutcome::Exited(42));
    assert_eq!(runtime.cpu.state.registers.esp, GUEST_STACK_TOP - 8);
    assert_eq!(
        runtime
            .memory
            .read_u32(GuestAddress(GUEST_TEB_BASE + 0x2c))
            .unwrap(),
        GUEST_TLS_BASE
    );
    assert_eq!(
        runtime
            .memory
            .read_u32(GuestAddress(GUEST_TLS_BASE))
            .unwrap(),
        0
    );
}

#[test]
fn resumes_a_suspended_host_call_after_guest_stdcall_callback() {
    #[derive(Debug, Clone, Copy)]
    struct CallbackHostCall;

    impl vnrt_win32::HostCallHandler for CallbackHostCall {
        fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
            context.set_return_u32(0xcafe_babe);
            context.set_stdcall_cleanup(8);
            context.request_guest_callback(GuestAddress(0x3000), &[0x1111, 0x2222])
        }
    }

    let image = image_with_one_import();
    let mut registry = ApiRegistry::new();
    registry.register(ApiKey::new("user32.dll", "MessageBoxA"), CallbackHostCall);
    let mut runtime = Runtime::load(&image, registry).expect("guest should load");
    runtime
        .memory
        .map_range(GuestAddress(0x3000), 0x2000, Permissions::ALL)
        .unwrap();
    runtime
        .memory
        .write(
            GuestAddress(0x3000),
            &[0xb8, 0x78, 0x56, 0x34, 0x12, 0xc2, 0x08, 0x00],
        )
        .unwrap();
    runtime.memory.write(GuestAddress(0x4000), &[0xf4]).unwrap();

    let host_stack = GUEST_STACK_TOP - 12;
    runtime.cpu.state.registers.esp = host_stack;
    runtime
        .memory
        .write_u32(GuestAddress(host_stack), 0x4000)
        .unwrap();
    runtime
        .dispatch_host_call(GuestAddress(HOST_THUNK_BASE))
        .expect("Host call should enter the Guest callback");

    assert_eq!(runtime.cpu.state.registers.eip, 0x3000);
    assert_eq!(runtime.cpu.state.registers.esp, host_stack - 12);
    assert_eq!(
        runtime
            .memory
            .read_u32(GuestAddress(runtime.cpu.state.registers.esp + 4))
            .unwrap(),
        0x1111
    );
    assert_eq!(
        runtime
            .memory
            .read_u32(GuestAddress(runtime.cpu.state.registers.esp + 8))
            .unwrap(),
        0x2222
    );

    assert_eq!(
        runtime
            .run(RunLimits {
                max_instructions: 8,
            })
            .expect("callback should return to the original caller"),
        RunOutcome::Halted
    );
    assert_eq!(runtime.cpu.state.registers.eax, 0xcafe_babe);
    assert_eq!(runtime.cpu.state.registers.esp, GUEST_STACK_TOP);
    assert!(runtime.suspended_host_calls.is_empty());
}

#[test]
fn runs_compiler_generated_pe32_fixture() {
    let image = include_bytes!("../../../tests/guest-programs/exit42.exe");
    let parsed_image = vnrt_pe::parse(image).expect("fixture metadata should parse");
    let mut registry = ApiRegistry::new();
    vnrt_kernel32::register(&mut registry);
    let config = RuntimeConfig {
        filesystem_root: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/guest-programs"),
        ..RuntimeConfig::default()
    };
    let mut runtime =
        Runtime::load_with_config(image, registry, config).expect("fixture should load");

    let outcome = runtime
        .run(RunLimits {
            max_instructions: 16_384,
        })
        .expect("compiler-generated guest should call ExitProcess");

    assert_eq!(outcome, RunOutcome::Exited(42));
    assert_host_thunk(&runtime, "kernel32.dll", "GetTickCount");
    assert_eq!(runtime.last_error, 1234);
    assert_eq!(runtime.cpu.state.fs_base, GUEST_TEB_BASE);
    assert!(parsed_image.tls.is_some());
    assert_eq!(
        runtime
            .memory
            .read_u32(GuestAddress(GUEST_TEB_BASE + 0x2c))
            .unwrap(),
        GUEST_TLS_BASE
    );
    assert_eq!(
        runtime
            .memory
            .read_u32(GuestAddress(GUEST_TLS_BASE))
            .unwrap(),
        GUEST_TLS_DATA_BASE
    );
    assert_eq!(
        runtime
            .memory
            .read_u32(GuestAddress(GUEST_TLS_DATA_BASE))
            .unwrap(),
        0x1357_9bdf
    );
    assert_eq!(
        runtime
            .memory
            .read_u8(GuestAddress(GUEST_TLS_DATA_BASE + 7))
            .unwrap(),
        0x5a
    );
    assert_eq!(
        runtime
            .memory
            .read_u8(GuestAddress(GUEST_TLS_DATA_BASE + 11))
            .unwrap(),
        0
    );
    let tls_index_address = parsed_image
        .tls
        .expect("fixture declares TLS")
        .address_of_index;
    assert_eq!(
        runtime
            .memory
            .read_u32(GuestAddress(tls_index_address))
            .unwrap(),
        0
    );
    assert_eq!(
        runtime
            .memory
            .read_u32(GuestAddress(GUEST_TEB_BASE + 0x18))
            .unwrap(),
        GUEST_TEB_BASE
    );
    assert_eq!(
        runtime
            .memory
            .read_u32(GuestAddress(GUEST_TEB_BASE + 0x30))
            .unwrap(),
        GUEST_PEB_BASE
    );
    assert_eq!(
        runtime
            .memory
            .read_u32(GuestAddress(GUEST_TEB_BASE + TEB_LAST_ERROR_OFFSET))
            .unwrap(),
        1234
    );
    assert!(
        runtime
            .memory
            .is_range_free(
                GuestAddress(GUEST_STACK_BASE - PAGE_SIZE_U32),
                PAGE_SIZE_U32
            )
            .unwrap()
    );
    assert_eq!(runtime.process_io.open_handle_count(), 0);
    assert_eq!(runtime.guest_stdout(), b"guest-ok\n");
    assert_eq!(runtime.heaps.live_allocation_count(), 0);
    assert!(runtime.virtual_memory.is_empty());
    assert!(
        runtime
            .memory
            .read_u8(GuestAddress(GUEST_HEAP_BASE))
            .is_ok()
    );
    let snapshot = runtime.diagnostic_snapshot();
    assert_eq!(snapshot.recent_host_calls.len(), 16);
    assert!(
        snapshot
            .recent_host_calls
            .iter()
            .any(|api| api.name.contains("GetCurrentDirectory"))
    );
    assert!(
        snapshot
            .recent_host_calls
            .iter()
            .any(|api| api.name.contains("EnvironmentStrings"))
    );
    assert_eq!(
        snapshot.recent_host_calls.last(),
        Some(&ApiKey::new("kernel32.dll", "ExitProcess"))
    );
    assert!(
        runtime
            .memory
            .is_range_free(GuestAddress(GUEST_VIRTUAL_BASE), PAGE_SIZE_U32)
            .unwrap()
    );
}

#[test]
fn runs_optimized_compiler_generated_pe32_fixture() {
    let image = include_bytes!("../../../tests/guest-programs/exit42-opt.exe");
    let mut registry = ApiRegistry::new();
    vnrt_kernel32::register(&mut registry);
    let config = RuntimeConfig {
        filesystem_root: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/guest-programs"),
        ..RuntimeConfig::default()
    };
    let mut runtime =
        Runtime::load_with_config(image, registry, config).expect("fixture should load");

    let outcome = runtime
        .run(RunLimits {
            max_instructions: 16_384,
        })
        .expect("optimized compiler-generated guest should call ExitProcess");

    assert_eq!(outcome, RunOutcome::Exited(42));
    assert_host_thunk(&runtime, "kernel32.dll", "GetTickCount");
    assert_eq!(runtime.guest_stdout(), b"guest-ok\n");
    assert_eq!(
        runtime
            .memory
            .read_u32(GuestAddress(GUEST_TEB_BASE + TEB_LAST_ERROR_OFFSET))
            .unwrap(),
        1234
    );
    assert_eq!(runtime.heaps.live_allocation_count(), 0);
    assert!(runtime.virtual_memory.is_empty());
}

fn image_with_one_import() -> Vec<u8> {
    let mut image = vec![0_u8; 0x400];
    image[0..2].copy_from_slice(&0x5a4d_u16.to_le_bytes());
    put_u32(&mut image, 0x3c, 0x80);
    put_u32(&mut image, 0x80, 0x0000_4550);
    image[0x84..0x86].copy_from_slice(&0x014c_u16.to_le_bytes());
    image[0x86..0x88].copy_from_slice(&1_u16.to_le_bytes());
    image[0x94..0x96].copy_from_slice(&0xe0_u16.to_le_bytes());
    image[0x98..0x9a].copy_from_slice(&0x010b_u16.to_le_bytes());
    put_u32(&mut image, 0xa8, 0x1000);
    put_u32(&mut image, 0xb4, 0x0040_0000);
    put_u32(&mut image, 0xb8, 0x1000);
    put_u32(&mut image, 0xbc, 0x200);
    put_u32(&mut image, 0xd0, 0x2000);
    put_u32(&mut image, 0xd4, 0x200);
    put_u32(&mut image, 0xf4, 16);
    put_u32(&mut image, 0x100, 0x1000);
    put_u32(&mut image, 0x104, 40);
    image[0x178..0x17e].copy_from_slice(b".idata");
    put_u32(&mut image, 0x180, 0x200);
    put_u32(&mut image, 0x184, 0x1000);
    put_u32(&mut image, 0x188, 0x200);
    put_u32(&mut image, 0x18c, 0x200);
    put_u32(&mut image, 0x19c, 0xc000_0040);
    put_u32(&mut image, 0x200, 0x1040);
    put_u32(&mut image, 0x20c, 0x1060);
    put_u32(&mut image, 0x210, 0x1050);
    put_u32(&mut image, 0x240, 0x1070);
    image[0x260..0x26b].copy_from_slice(b"USER32.dll\0");
    image[0x270..0x27e].copy_from_slice(b"\0\0MessageBoxA\0");
    image
}

fn assert_host_thunk(runtime: &Runtime, module: &str, name: &str) {
    let expected = ApiKey::new(module, name);
    assert!(
        runtime.import_thunks.values().any(|api| api == &expected),
        "missing generated Host thunk for {module}!{name}"
    );
}

fn image_that_calls_exit_process(exit_code: u8) -> Vec<u8> {
    let mut image = vec![0_u8; 0x600];
    image[0..2].copy_from_slice(&0x5a4d_u16.to_le_bytes());
    put_u32(&mut image, 0x3c, 0x80);
    put_u32(&mut image, 0x80, 0x0000_4550);
    image[0x84..0x86].copy_from_slice(&0x014c_u16.to_le_bytes());
    image[0x86..0x88].copy_from_slice(&2_u16.to_le_bytes());
    image[0x94..0x96].copy_from_slice(&0xe0_u16.to_le_bytes());
    image[0x98..0x9a].copy_from_slice(&0x010b_u16.to_le_bytes());
    put_u32(&mut image, 0xa8, 0x1000);
    put_u32(&mut image, 0xb4, 0x0040_0000);
    put_u32(&mut image, 0xb8, 0x1000);
    put_u32(&mut image, 0xbc, 0x200);
    put_u32(&mut image, 0xd0, 0x3000);
    put_u32(&mut image, 0xd4, 0x200);
    put_u32(&mut image, 0xf4, 16);
    put_u32(&mut image, 0x100, 0x2000);
    put_u32(&mut image, 0x104, 40);

    image[0x178..0x17d].copy_from_slice(b".text");
    put_u32(&mut image, 0x180, 0x200);
    put_u32(&mut image, 0x184, 0x1000);
    put_u32(&mut image, 0x188, 0x200);
    put_u32(&mut image, 0x18c, 0x200);
    put_u32(&mut image, 0x19c, 0x6000_0020);

    image[0x1a0..0x1a6].copy_from_slice(b".idata");
    put_u32(&mut image, 0x1a8, 0x200);
    put_u32(&mut image, 0x1ac, 0x2000);
    put_u32(&mut image, 0x1b0, 0x200);
    put_u32(&mut image, 0x1b4, 0x400);
    put_u32(&mut image, 0x1c4, 0xc000_0040);

    image[0x200..0x208].copy_from_slice(&[0x6a, exit_code, 0xff, 0x15, 0x50, 0x20, 0x40, 0x00]);
    put_u32(&mut image, 0x400, 0x2040);
    put_u32(&mut image, 0x40c, 0x2060);
    put_u32(&mut image, 0x410, 0x2050);
    put_u32(&mut image, 0x440, 0x2070);
    image[0x460..0x46d].copy_from_slice(b"KERNEL32.dll\0");
    image[0x470..0x47e].copy_from_slice(b"\0\0ExitProcess\0");
    image
}

fn put_u32(image: &mut [u8], offset: usize, value: u32) {
    image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
