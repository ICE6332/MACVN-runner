use super::*;

pub(super) struct RuntimeHostContext<'a> {
    pub(super) cpu: &'a mut Interpreter,
    pub(super) memory: &'a mut GuestMemory,
    pub(super) exit_code: &'a mut Option<u32>,
    pub(super) stdcall_cleanup: u32,
    pub(super) started_at: Instant,
    pub(super) heaps: &'a mut GuestHeapManager,
    pub(super) global_allocations: &'a mut BTreeMap<u32, u32>,
    pub(super) tls_slots: &'a mut TlsSlotManager,
    pub(super) unhandled_exception_filter: &'a mut u32,
    pub(super) mutexes: &'a mut MutexManager,
    pub(super) events: &'a mut EventManager,
    pub(super) tokens: &'a mut TokenManager,
    pub(super) com_initialization_count: &'a mut u32,
    pub(super) cursor_display_count: &'a mut i32,
    pub(super) focused_window: &'a mut u32,
    pub(super) window_class_longs: &'a mut HashMap<(u32, i32), u32>,
    pub(super) icons: &'a mut BTreeSet<u32>,
    pub(super) next_icon_handle: &'a mut u32,
    pub(super) window_classes: &'a mut HashMap<String, (u16, GuestAddress)>,
    pub(super) next_window_class_atom: &'a mut u16,
    pub(super) window_regions: &'a mut HashMap<u32, u32>,
    pub(super) windows: &'a mut HashMap<u32, String>,
    pub(super) window_titles: &'a mut HashMap<u32, String>,
    pub(super) visible_windows: &'a mut BTreeSet<u32>,
    pub(super) window_placements: &'a mut HashMap<u32, Vec<u8>>,
    pub(super) disabled_windows: &'a mut BTreeSet<u32>,
    pub(super) thread_messages: &'a mut VecDeque<(u32, u32, u32, u32)>,
    pub(super) primary_display_size: &'a mut (u32, u32),
    pub(super) menus: &'a mut BTreeSet<u32>,
    pub(super) next_menu_handle: &'a mut u32,
    pub(super) menu_children: &'a mut HashMap<u32, Vec<u32>>,
    pub(super) cursor_position: &'a mut (i32, i32),
    pub(super) window_menus: &'a mut HashMap<u32, u32>,
    pub(super) clipboard_open: &'a mut bool,
    pub(super) clipboard_data: &'a mut HashMap<u32, u32>,
    pub(super) window_longs: &'a mut HashMap<(u32, i32), u32>,
    pub(super) invalidated_windows: &'a mut BTreeSet<u32>,
    pub(super) window_dcs: &'a mut HashMap<u32, u32>,
    pub(super) next_window_dc: &'a mut u32,
    pub(super) keyboard_state: &'a mut [u8; 256],
    pub(super) memory_dcs: &'a mut BTreeSet<u32>,
    pub(super) next_memory_dc: &'a mut u32,
    pub(super) selected_gdi_objects: &'a mut HashMap<u32, u32>,
    pub(super) gdi_objects: &'a mut HashMap<u32, Vec<u8>>,
    pub(super) next_gdi_object: &'a mut u32,
    pub(super) gdi_dc_attributes: &'a mut HashMap<(u32, u32), u32>,
    pub(super) window_frames: &'a mut HashMap<u32, WindowFrame>,
    pub(super) next_window_handle: &'a mut u32,
    pub(super) image_base: GuestAddress,
    pub(super) resource_directory: Option<(GuestAddress, u32)>,
    pub(super) virtual_memory: &'a mut GuestRegionAllocator,
    pub(super) api_registry: &'a ApiRegistry,
    pub(super) import_thunks: &'a mut HashMap<GuestAddress, ApiKey>,
    pub(super) host_modules: &'a HashMap<String, GuestAddress>,
    pub(super) command_line_ansi: GuestAddress,
    pub(super) command_line_utf16: GuestAddress,
    pub(super) process_parameters: GuestAddress,
    pub(super) module_path: &'a str,
    pub(super) last_error: &'a mut u32,
    pub(super) error_mode: &'a mut u32,
    pub(super) process_io: &'a mut ProcessIo,
    pub(super) guest_stdout: &'a mut Vec<u8>,
    pub(super) guest_stderr: &'a mut Vec<u8>,
    pub(super) standard_handles: &'a mut [u32; 3],
    pub(super) environment: &'a mut BTreeMap<String, String>,
    pub(super) current_directory: &'a mut String,
    pub(super) environment_block_ansi: &'a mut GuestAddress,
    pub(super) environment_block_utf16: &'a mut GuestAddress,
    pub(super) guest_callbacks: VecDeque<GuestCallback>,
    pub(super) suspended_host_calls: &'a mut Vec<SuspendedHostCall>,
    pub(super) guest_callback_targets: &'a mut HashMap<u32, GuestAddress>,
    pub(super) capture_callback_return: bool,
    pub(super) raised_exception: Option<(u32, u32, Vec<u32>)>,
}

impl RuntimeHostContext<'_> {
    pub(super) fn finish_host_return(&mut self) -> Result<(), RuntimeError> {
        let stack = self.cpu.state.registers.esp;
        let return_address = self.memory.read_u32(GuestAddress(stack))?;
        self.cpu.state.registers.esp = stack
            .checked_add(4)
            .and_then(|value| value.checked_add(self.stdcall_cleanup))
            .ok_or(RuntimeError::Unsupported("stdcall stack pointer overflow"))?;
        self.cpu.state.registers.eip = return_address;
        Ok(())
    }
}

fn store_dib_frame(
    context: &mut RuntimeHostContext<'_>,
    destination: u32,
    width: u32,
    signed_height: i32,
    stride: u32,
    bits_per_pixel: u16,
    pixels: GuestAddress,
) -> Result<bool, Win32Error> {
    let Some(window) = context
        .window_dcs
        .iter()
        .find_map(|(window, dc)| (*dc == destination).then_some(*window))
    else {
        return Ok(false);
    };
    let height = signed_height.unsigned_abs();
    if width == 0 || height == 0 || pixels.0 == 0 || !matches!(bits_per_pixel, 24 | 32) {
        return Ok(false);
    }
    let byte_count = stride
        .checked_mul(height)
        .filter(|size| *size <= 256 * 1024 * 1024)
        .ok_or(Win32Error::InvalidArgument("presented bitmap size"))?;
    let mut source_bytes = vec![0; byte_count as usize];
    context.read_memory(pixels, &mut source_bytes)?;
    let output_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(Win32Error::InvalidArgument("presented RGBA size"))?;
    let mut rgba = vec![0; output_len as usize];
    let source_pixel_size = usize::from(bits_per_pixel / 8);
    for y in 0..height as usize {
        let source_y = if signed_height > 0 {
            height as usize - 1 - y
        } else {
            y
        };
        let row_start = source_y * stride as usize;
        for x in 0..width as usize {
            let source_offset = row_start + x * source_pixel_size;
            let output_offset = (y * width as usize + x) * 4;
            rgba[output_offset] = source_bytes[source_offset + 2];
            rgba[output_offset + 1] = source_bytes[source_offset + 1];
            rgba[output_offset + 2] = source_bytes[source_offset];
            rgba[output_offset + 3] = 255;
        }
    }
    context.window_frames.insert(
        window,
        WindowFrame {
            width,
            height,
            rgba,
        },
    );
    Ok(true)
}

impl HostCallContext for RuntimeHostContext<'_> {
    fn argument_u32(&self, index: usize) -> Result<u32, Win32Error> {
        let byte_offset = u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .and_then(|value| value.checked_add(4))
            .ok_or(Win32Error::InvalidArgument(
                "stdcall argument index overflow",
            ))?;
        let address = self
            .cpu
            .state
            .registers
            .esp
            .checked_add(byte_offset)
            .ok_or(Win32Error::InvalidArgument(
                "stdcall argument address overflow",
            ))?;
        self.memory
            .read_u32(GuestAddress(address))
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))
    }

    fn set_return_u32(&mut self, value: u32) {
        self.cpu.state.registers.eax = value;
    }

    fn set_stdcall_cleanup(&mut self, argument_bytes: u32) {
        self.stdcall_cleanup = argument_bytes;
    }

    fn request_guest_callback(
        &mut self,
        callback: GuestAddress,
        arguments: &[u32],
    ) -> Result<(), Win32Error> {
        if callback.0 == 0 {
            return Err(Win32Error::InvalidArgument("null Guest callback"));
        }
        if arguments.len() > MAX_HOST_CALLBACK_ARGUMENTS {
            return Err(Win32Error::InvalidArgument(
                "Guest callback argument limit exceeded",
            ));
        }
        self.guest_callbacks.push_back(GuestCallback {
            target: callback,
            arguments: arguments.to_vec(),
        });
        Ok(())
    }

    fn use_guest_callback_return_value(&mut self) {
        self.capture_callback_return = true;
    }

    fn complete_suspended_host_call(&mut self, return_value: u32) -> Result<(), Win32Error> {
        let continuation =
            self.suspended_host_calls
                .last_mut()
                .ok_or(Win32Error::InvalidArgument(
                    "no suspended Host call for Guest callback",
                ))?;
        continuation.return_value = return_value;
        continuation.callbacks.clear();
        Ok(())
    }

    fn register_guest_callback_target(&mut self, object: u32, callback: GuestAddress) {
        self.guest_callback_targets.insert(object, callback);
    }

    fn guest_callback_target(&self, object: u32) -> Option<GuestAddress> {
        self.guest_callback_targets.get(&object).copied()
    }

    fn replace_focus_window(&mut self, window: u32) -> u32 {
        std::mem::replace(self.focused_window, window)
    }

    fn focused_window(&self) -> u32 {
        *self.focused_window
    }

    fn replace_window_class_long(&mut self, window: u32, index: i32, value: u32) -> u32 {
        self.window_class_longs
            .insert((window, index), value)
            .unwrap_or(0)
    }

    fn window_class_long(&self, window: u32, index: i32) -> u32 {
        self.window_class_longs
            .get(&(window, index))
            .copied()
            .unwrap_or(0)
    }

    fn create_icon(&mut self) -> u32 {
        let handle = *self.next_icon_handle;
        *self.next_icon_handle = self.next_icon_handle.wrapping_add(4);
        self.icons.insert(handle);
        handle
    }

    fn destroy_icon(&mut self, icon: u32) -> bool {
        self.icons.remove(&icon)
    }

    fn register_window_class(&mut self, name: &str, callback: GuestAddress) -> Option<u16> {
        let key = name.to_ascii_lowercase();
        if self.window_classes.contains_key(&key) {
            return None;
        }
        let atom = *self.next_window_class_atom;
        *self.next_window_class_atom = self.next_window_class_atom.wrapping_add(1).max(0xc000);
        self.window_classes.insert(key, (atom, callback));
        Some(atom)
    }

    fn window_class_callback_by_name(&self, name: &str) -> Option<GuestAddress> {
        self.window_classes
            .get(&name.to_ascii_lowercase())
            .map(|(_, callback)| *callback)
    }

    fn window_class_callback_by_atom(&self, atom: u16) -> Option<GuestAddress> {
        self.window_classes
            .values()
            .find_map(|(candidate, callback)| (*candidate == atom).then_some(*callback))
    }

    fn window_class_name_by_atom(&self, atom: u16) -> Option<String> {
        self.window_classes
            .iter()
            .find_map(|(name, (candidate, _))| (*candidate == atom).then(|| name.clone()))
    }

    fn create_window(&mut self, class_name: &str, title: &str, visible: bool) -> u32 {
        let handle = *self.next_window_handle;
        *self.next_window_handle = self.next_window_handle.wrapping_add(4);
        self.windows.insert(handle, class_name.to_owned());
        self.window_titles.insert(handle, title.to_owned());
        if visible {
            self.visible_windows.insert(handle);
            self.invalidated_windows.insert(handle);
        }
        handle
    }

    fn window_class_name(&self, window: u32) -> Option<String> {
        self.windows.get(&window).cloned()
    }

    fn window_title(&self, window: u32) -> Option<String> {
        self.window_titles.get(&window).cloned()
    }

    fn set_window_title(&mut self, window: u32, title: &str) -> bool {
        if !self.windows.contains_key(&window) {
            return false;
        }
        self.window_titles.insert(window, title.to_owned());
        true
    }

    fn remove_window(&mut self, window: u32) -> bool {
        self.window_regions.remove(&window);
        self.window_titles.remove(&window);
        self.window_menus.remove(&window);
        self.visible_windows.remove(&window);
        self.window_placements.remove(&window);
        self.disabled_windows.remove(&window);
        self.window_class_longs
            .retain(|(hwnd, _), _| *hwnd != window);
        self.window_longs.retain(|(hwnd, _), _| *hwnd != window);
        self.invalidated_windows.remove(&window);
        self.window_dcs.remove(&window);
        self.guest_callback_targets.remove(&window);
        self.windows.remove(&window).is_some()
    }

    fn is_window(&self, window: u32) -> bool {
        self.windows.contains_key(&window)
    }

    fn is_window_visible(&self, window: u32) -> bool {
        self.visible_windows.contains(&window)
    }

    fn set_window_visible(&mut self, window: u32, visible: bool) -> bool {
        let previous = self.visible_windows.contains(&window);
        if self.windows.contains_key(&window) {
            if visible {
                self.visible_windows.insert(window);
            } else {
                self.visible_windows.remove(&window);
            }
        }
        previous
    }

    fn set_window_placement(&mut self, window: u32, placement: &[u8]) -> bool {
        if !self.windows.contains_key(&window) {
            return false;
        }
        self.window_placements.insert(window, placement.to_vec());
        true
    }

    fn window_placement(&self, window: u32) -> Option<Vec<u8>> {
        if !self.windows.contains_key(&window) {
            return None;
        }
        Some(
            self.window_placements
                .get(&window)
                .cloned()
                .unwrap_or_else(|| {
                    let mut placement = vec![0; 44];
                    placement[0..4].copy_from_slice(&44_u32.to_le_bytes());
                    placement[8..12].copy_from_slice(&1_u32.to_le_bytes());
                    placement
                }),
        )
    }

    fn set_window_enabled(&mut self, window: u32, enabled: bool) -> bool {
        let previous =
            self.windows.contains_key(&window) && !self.disabled_windows.contains(&window);
        if self.windows.contains_key(&window) {
            if enabled {
                self.disabled_windows.remove(&window);
            } else {
                self.disabled_windows.insert(window);
            }
        }
        previous
    }

    fn is_window_enabled(&self, window: u32) -> bool {
        self.windows.contains_key(&window) && !self.disabled_windows.contains(&window)
    }

    fn post_thread_message(&mut self, window: u32, message: u32, wparam: u32, lparam: u32) {
        self.thread_messages
            .push_back((window, message, wparam, lparam));
    }

    fn next_thread_message(
        &mut self,
        remove: bool,
        minimum: u32,
        maximum: u32,
    ) -> Option<(u32, u32, u32, u32)> {
        let matches = |message: u32| {
            message == 0x0012
                || (minimum == 0 && maximum == 0)
                || (minimum <= message && message <= maximum)
        };
        let index = self
            .thread_messages
            .iter()
            .position(|(_, message, _, _)| matches(*message))?;
        if remove {
            self.thread_messages.remove(index)
        } else {
            self.thread_messages.get(index).copied()
        }
    }

    fn primary_display_size(&self) -> (u32, u32) {
        *self.primary_display_size
    }

    fn set_primary_display_size(&mut self, width: u32, height: u32) {
        *self.primary_display_size = (width, height);
    }

    fn create_menu(&mut self) -> u32 {
        let handle = *self.next_menu_handle;
        *self.next_menu_handle = self.next_menu_handle.wrapping_add(4);
        self.menus.insert(handle);
        handle
    }

    fn destroy_menu(&mut self, menu: u32) -> bool {
        self.menu_children.remove(&menu);
        self.menus.remove(&menu)
    }

    fn is_menu(&self, menu: u32) -> bool {
        self.menus.contains(&menu)
    }

    fn insert_submenu(&mut self, menu: u32, position: usize, submenu: u32) -> bool {
        if !self.menus.contains(&menu) {
            return false;
        }
        let children = self.menu_children.entry(menu).or_default();
        let position = position.min(children.len());
        children.insert(position, submenu);
        true
    }

    fn submenu(&self, menu: u32, position: usize) -> Option<u32> {
        self.menu_children
            .get(&menu)
            .and_then(|children| children.get(position))
            .copied()
            .filter(|submenu| *submenu != 0)
    }

    fn window_handles(&self) -> Vec<u32> {
        let mut windows = self.windows.keys().copied().collect::<Vec<_>>();
        windows.sort_unstable();
        windows
    }

    fn cursor_position(&self) -> (i32, i32) {
        *self.cursor_position
    }

    fn set_cursor_position(&mut self, x: i32, y: i32) {
        *self.cursor_position = (x, y);
    }

    fn set_window_menu(&mut self, window: u32, menu: u32) -> bool {
        if !self.windows.contains_key(&window) {
            return false;
        }
        if menu == 0 {
            self.window_menus.remove(&window);
        } else {
            self.window_menus.insert(window, menu);
        }
        true
    }

    fn window_menu(&self, window: u32) -> Option<u32> {
        self.window_menus.get(&window).copied()
    }

    fn set_clipboard_open(&mut self, open: bool) -> bool {
        if *self.clipboard_open == open {
            return false;
        }
        *self.clipboard_open = open;
        true
    }

    fn clear_clipboard(&mut self) {
        self.clipboard_data.clear();
    }

    fn clipboard_data(&self, format: u32) -> Option<u32> {
        self.clipboard_data.get(&format).copied()
    }

    fn set_clipboard_data(&mut self, format: u32, handle: u32) {
        self.clipboard_data.insert(format, handle);
    }

    fn replace_window_long(&mut self, window: u32, index: i32, value: u32) -> Option<u32> {
        if !self.windows.contains_key(&window) {
            return None;
        }
        if index == -4 {
            self.guest_callback_targets
                .insert(window, GuestAddress(value));
        }
        Some(
            self.window_longs
                .insert((window, index), value)
                .unwrap_or(0),
        )
    }

    fn window_long(&self, window: u32, index: i32) -> Option<u32> {
        self.windows.contains_key(&window).then(|| {
            self.window_longs
                .get(&(window, index))
                .copied()
                .unwrap_or(0)
        })
    }

    fn invalidate_window(&mut self, window: u32) -> bool {
        self.windows.contains_key(&window) && self.invalidated_windows.insert(window)
    }

    fn validate_window(&mut self, window: u32) -> bool {
        self.windows.contains_key(&window) && self.invalidated_windows.remove(&window)
    }

    fn window_needs_paint(&self, window: u32) -> bool {
        self.windows.contains_key(&window) && self.invalidated_windows.contains(&window)
    }

    fn window_dc(&mut self, window: u32) -> Option<u32> {
        if !self.windows.contains_key(&window) {
            return None;
        }
        if let Some(dc) = self.window_dcs.get(&window).copied() {
            return Some(dc);
        }
        let dc = *self.next_window_dc;
        *self.next_window_dc = self.next_window_dc.wrapping_add(4);
        self.window_dcs.insert(window, dc);
        Some(dc)
    }

    fn is_window_dc(&self, dc: u32) -> bool {
        self.window_dcs.values().any(|candidate| *candidate == dc)
    }

    fn keyboard_state(&self) -> [u8; 256] {
        *self.keyboard_state
    }

    fn set_keyboard_state(&mut self, state: &[u8; 256]) {
        *self.keyboard_state = *state;
    }

    fn is_gdi_dc(&self, dc: u32) -> bool {
        dc == 0x0003_0000
            || self.window_dcs.values().any(|candidate| *candidate == dc)
            || self.memory_dcs.contains(&dc)
    }

    fn create_memory_dc(&mut self, source: u32) -> Option<u32> {
        if source != 0 && !self.is_gdi_dc(source) {
            return None;
        }
        let dc = *self.next_memory_dc;
        *self.next_memory_dc = self.next_memory_dc.wrapping_add(4);
        self.memory_dcs.insert(dc);
        Some(dc)
    }

    fn delete_memory_dc(&mut self, dc: u32) -> bool {
        self.selected_gdi_objects.remove(&dc);
        self.gdi_dc_attributes
            .retain(|(candidate, _), _| *candidate != dc);
        self.memory_dcs.remove(&dc)
    }

    fn select_gdi_object(&mut self, dc: u32, object: u32) -> Option<u32> {
        if !self.is_gdi_dc(dc) || object == 0 {
            return None;
        }
        Some(self.selected_gdi_objects.insert(dc, object).unwrap_or(0))
    }

    fn selected_gdi_object(&self, dc: u32) -> Option<u32> {
        self.selected_gdi_objects.get(&dc).copied()
    }

    fn create_gdi_object(&mut self, descriptor: &[u8]) -> u32 {
        let object = *self.next_gdi_object;
        *self.next_gdi_object = self.next_gdi_object.wrapping_add(4);
        self.gdi_objects.insert(object, descriptor.to_vec());
        object
    }

    fn gdi_object(&self, object: u32) -> Option<Vec<u8>> {
        self.gdi_objects.get(&object).cloned()
    }

    fn delete_gdi_object(&mut self, object: u32) -> bool {
        if self
            .selected_gdi_objects
            .values()
            .any(|selected| *selected == object)
        {
            return false;
        }
        self.gdi_objects.remove(&object).is_some()
    }

    fn replace_gdi_dc_attribute(
        &mut self,
        dc: u32,
        attribute: u32,
        value: u32,
        default: u32,
    ) -> Option<u32> {
        if !self.is_gdi_dc(dc) {
            return None;
        }
        Some(
            self.gdi_dc_attributes
                .insert((dc, attribute), value)
                .unwrap_or(default),
        )
    }

    fn present_selected_bitmap(
        &mut self,
        destination: u32,
        source: u32,
    ) -> Result<bool, Win32Error> {
        let Some(object) = self.selected_gdi_objects.get(&source).copied() else {
            return Ok(false);
        };
        let Some(descriptor) = self.gdi_objects.get(&object) else {
            return Ok(false);
        };
        if descriptor.len() < 24 {
            return Ok(false);
        }
        let width = u32::from_le_bytes(descriptor[4..8].try_into().expect("bitmap width"));
        let signed_height =
            i32::from_le_bytes(descriptor[8..12].try_into().expect("bitmap height"));
        let stride = u32::from_le_bytes(descriptor[12..16].try_into().expect("bitmap stride"));
        let bits_per_pixel =
            u16::from_le_bytes(descriptor[18..20].try_into().expect("bitmap depth"));
        let pixels = GuestAddress(u32::from_le_bytes(
            descriptor[20..24].try_into().expect("bitmap pixels"),
        ));
        store_dib_frame(
            self,
            destination,
            width,
            signed_height,
            stride,
            bits_per_pixel,
            pixels,
        )
    }

    fn present_dib(
        &mut self,
        destination: u32,
        width: u32,
        height: i32,
        stride: u32,
        bits_per_pixel: u16,
        pixels: GuestAddress,
    ) -> Result<bool, Win32Error> {
        store_dib_frame(
            self,
            destination,
            width,
            height,
            stride,
            bits_per_pixel,
            pixels,
        )
    }

    fn set_window_region(&mut self, window: u32, region: u32) {
        if region == 0 {
            self.window_regions.remove(&window);
        } else {
            self.window_regions.insert(window, region);
        }
    }

    fn read_memory(&self, address: GuestAddress, output: &mut [u8]) -> Result<(), Win32Error> {
        self.memory
            .read(address, output)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))
    }

    fn write_memory(&mut self, address: GuestAddress, bytes: &[u8]) -> Result<(), Win32Error> {
        self.memory
            .write(address, bytes)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))
    }

    fn request_exit(&mut self, code: u32) {
        *self.exit_code = Some(code);
        self.cpu.state.halted = true;
    }

    fn raise_guest_exception(
        &mut self,
        code: u32,
        flags: u32,
        information: &[u32],
    ) -> Result<(), Win32Error> {
        if self.raised_exception.is_some() {
            return Err(Win32Error::InvalidArgument(
                "multiple exceptions from one Host call",
            ));
        }
        self.raised_exception = Some((code, flags, information.to_vec()));
        Ok(())
    }

    fn tick_count(&self) -> u32 {
        self.started_at.elapsed().as_millis() as u32
    }

    fn performance_counter(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    fn performance_frequency(&self) -> u64 {
        1_000_000_000
    }

    fn system_time_filetime(&self) -> u64 {
        const WINDOWS_TO_UNIX_SECONDS: u64 = 11_644_473_600;
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        WINDOWS_TO_UNIX_SECONDS
            .saturating_add(unix.as_secs())
            .saturating_mul(10_000_000)
            .saturating_add(u64::from(unix.subsec_nanos() / 100))
    }

    fn create_heap(
        &mut self,
        initial_size: u32,
        maximum_size: u32,
        executable: bool,
    ) -> Result<Handle, Win32Error> {
        self.heaps.create(initial_size, maximum_size, executable)
    }

    fn destroy_heap(&mut self, heap: Handle) -> Result<(), Win32Error> {
        self.heaps.destroy(self.memory, heap)
    }

    fn allocate_heap_memory(
        &mut self,
        heap: Handle,
        size: u32,
    ) -> Result<GuestAddress, Win32Error> {
        self.heaps.allocate(self.memory, heap, size)
    }

    fn reallocate_heap_memory(
        &mut self,
        heap: Handle,
        address: GuestAddress,
        size: u32,
    ) -> Result<GuestAddress, Win32Error> {
        self.heaps.reallocate(self.memory, heap, address, size)
    }

    fn free_heap_memory(&mut self, heap: Handle, address: GuestAddress) -> Result<(), Win32Error> {
        self.heaps.free(self.memory, heap, address)
    }

    fn heap_memory_size(&self, heap: Handle, address: GuestAddress) -> Result<u32, Win32Error> {
        self.heaps.size(heap, address)
    }

    fn allocate_global_memory(&mut self, size: u32) -> Result<Handle, Win32Error> {
        let address = self
            .heaps
            .allocate(self.memory, Handle(PROCESS_HEAP_HANDLE), size)?;
        self.global_allocations.insert(address.0, 0);
        Ok(Handle(address.0))
    }

    fn lock_global_memory(&mut self, handle: Handle) -> Result<GuestAddress, Win32Error> {
        let lock_count = self
            .global_allocations
            .get_mut(&handle.0)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        *lock_count = lock_count.checked_add(1).ok_or(Win32Error::OutOfMemory)?;
        Ok(GuestAddress(handle.0))
    }

    fn unlock_global_memory(&mut self, handle: Handle) -> Result<bool, Win32Error> {
        let lock_count = self
            .global_allocations
            .get_mut(&handle.0)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        if *lock_count == 0 {
            return Err(Win32Error::InvalidArgument("GlobalUnlock without lock"));
        }
        *lock_count -= 1;
        Ok(*lock_count != 0)
    }

    fn free_global_memory(&mut self, handle: Handle) -> Result<(), Win32Error> {
        self.global_allocations
            .remove(&handle.0)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        self.heaps.free(
            self.memory,
            Handle(PROCESS_HEAP_HANDLE),
            GuestAddress(handle.0),
        )
    }

    fn allocate_tls_index(&mut self) -> Result<u32, Win32Error> {
        self.tls_slots.allocate(self.memory)
    }

    fn free_tls_index(&mut self, index: u32) -> Result<(), Win32Error> {
        self.tls_slots.free(self.memory, index)
    }

    fn tls_value(&self, index: u32) -> Result<u32, Win32Error> {
        self.tls_slots.get(self.memory, index)
    }

    fn set_tls_value(&mut self, index: u32, value: u32) -> Result<(), Win32Error> {
        self.tls_slots.set(self.memory, index, value)
    }

    fn replace_unhandled_exception_filter(&mut self, filter: u32) -> u32 {
        std::mem::replace(self.unhandled_exception_filter, filter)
    }

    fn unhandled_exception_filter(&self) -> GuestAddress {
        GuestAddress(*self.unhandled_exception_filter)
    }

    fn initialize_com(&mut self) -> u32 {
        let result = u32::from(*self.com_initialization_count != 0); // S_OK / S_FALSE
        *self.com_initialization_count = self.com_initialization_count.saturating_add(1);
        result
    }

    fn uninitialize_com(&mut self) {
        *self.com_initialization_count = self.com_initialization_count.saturating_sub(1);
    }

    fn adjust_cursor_display_count(&mut self, show: bool) -> i32 {
        *self.cursor_display_count = if show {
            self.cursor_display_count.saturating_add(1)
        } else {
            self.cursor_display_count.saturating_sub(1)
        };
        *self.cursor_display_count
    }

    fn main_module_base(&self) -> GuestAddress {
        self.image_base
    }

    fn resource_directory(&self) -> Option<(GuestAddress, u32)> {
        self.resource_directory
    }

    fn allocate_virtual_memory(
        &mut self,
        size: u32,
        read: bool,
        write: bool,
        execute: bool,
    ) -> Result<GuestAddress, Win32Error> {
        self.virtual_memory
            .allocate(self.memory, size, Permissions::new(read, write, execute))
    }

    fn free_virtual_memory(&mut self, address: GuestAddress) -> Result<(), Win32Error> {
        self.virtual_memory.free(self.memory, address)
    }

    fn reserve_virtual_memory(&mut self, size: u32) -> Result<GuestAddress, Win32Error> {
        self.virtual_memory.reserve(self.memory, size)
    }

    fn commit_virtual_memory(
        &mut self,
        address: GuestAddress,
        size: u32,
        read: bool,
        write: bool,
        execute: bool,
    ) -> Result<(), Win32Error> {
        self.virtual_memory.commit(
            self.memory,
            address,
            size,
            Permissions::new(read, write, execute),
        )
    }

    fn protect_virtual_memory(
        &mut self,
        address: GuestAddress,
        size: u32,
        read: bool,
        write: bool,
        execute: bool,
    ) -> Result<(bool, bool, bool), Win32Error> {
        if size == 0 {
            return Err(Win32Error::InvalidArgument("zero virtual protection size"));
        }
        let start = address.0 & !(PAGE_SIZE_U32 - 1);
        let end = address
            .0
            .checked_add(size)
            .and_then(|end| align_up(end, PAGE_SIZE_U32))
            .ok_or(Win32Error::OutOfMemory)?;
        let old = self
            .memory
            .permissions_at(GuestAddress(start))
            .ok_or(Win32Error::InvalidAllocation { address: address.0 })?;
        self.memory
            .protect_range(
                GuestAddress(start),
                end - start,
                Permissions::new(read, write, execute),
            )
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;
        Ok((old.read, old.write, old.execute))
    }

    fn is_memory_writable(&self, address: GuestAddress, size: u32) -> bool {
        if size == 0 {
            return true;
        }
        let Some(last) = address.0.checked_add(size - 1) else {
            return false;
        };
        let mut page = address.page_base().0;
        let last_page = GuestAddress(last).page_base().0;
        loop {
            if !self
                .memory
                .permissions_at(GuestAddress(page))
                .is_some_and(|permissions| permissions.write)
            {
                return false;
            }
            if page == last_page {
                return true;
            }
            let Some(next) = page.checked_add(PAGE_SIZE_U32) else {
                return false;
            };
            page = next;
        }
    }

    fn is_memory_readable(&self, address: GuestAddress, size: u32) -> bool {
        if size == 0 {
            return true;
        }
        let Some(last) = address.0.checked_add(size - 1) else {
            return false;
        };
        let mut page = address.page_base().0;
        let last_page = GuestAddress(last).page_base().0;
        loop {
            if !self
                .memory
                .permissions_at(GuestAddress(page))
                .is_some_and(|permissions| permissions.read)
            {
                return false;
            }
            if page == last_page {
                return true;
            }
            let Some(next) = page.checked_add(PAGE_SIZE_U32) else {
                return false;
            };
            page = next;
        }
    }

    fn is_memory_executable(&self, address: GuestAddress) -> bool {
        self.memory
            .permissions_at(address)
            .is_some_and(|permissions| permissions.execute)
    }

    fn loaded_module_handle(&self, name: &str) -> Option<GuestAddress> {
        let mut normalized = name.to_ascii_lowercase();
        if !normalized.contains('.') {
            normalized.push_str(".dll");
        }
        self.host_modules.get(&normalized).copied()
    }

    fn loaded_module_name(&self, module: GuestAddress) -> Option<String> {
        self.host_modules
            .iter()
            .find_map(|(name, handle)| (*handle == module).then(|| name.clone()))
    }

    fn resolve_host_api(
        &mut self,
        module: GuestAddress,
        name: &str,
    ) -> Result<GuestAddress, Win32Error> {
        let module_name = self
            .host_modules
            .iter()
            .find_map(|(name, handle)| (*handle == module).then(|| name.clone()))
            .ok_or_else(|| Win32Error::ModuleNotFound(format!("{:#010x}", module.0)))?;
        let key = ApiKey::new(module_name.clone(), name);
        if self.api_registry.resolve(&key).is_none() {
            return Err(Win32Error::ProcedureNotFound {
                module: module_name,
                name: name.to_owned(),
            });
        }
        if let Some((address, _)) = self
            .import_thunks
            .iter()
            .find(|(_, existing)| **existing == key)
        {
            return Ok(*address);
        }
        let index = u32::try_from(self.import_thunks.len()).map_err(|_| Win32Error::OutOfMemory)?;
        let address = HOST_THUNK_BASE
            .checked_add(index.checked_mul(4).ok_or(Win32Error::OutOfMemory)?)
            .map(GuestAddress)
            .ok_or(Win32Error::OutOfMemory)?;
        self.import_thunks.insert(address, key);
        Ok(address)
    }

    fn command_line_ansi(&self) -> GuestAddress {
        self.command_line_ansi
    }

    fn command_line_utf16(&self) -> GuestAddress {
        self.command_line_utf16
    }

    fn main_module_path(&self) -> &str {
        self.module_path
    }

    fn last_error(&self) -> u32 {
        *self.last_error
    }

    fn set_last_error(&mut self, value: u32) {
        *self.last_error = value;
    }

    fn replace_process_error_mode(&mut self, mode: u32) -> u32 {
        std::mem::replace(self.error_mode, mode)
    }

    fn open_file_read(&mut self, path: &str) -> Result<Handle, Win32Error> {
        self.process_io.open_read(path)
    }

    fn open_file(
        &mut self,
        path: &str,
        readable: bool,
        writable: bool,
        disposition: u32,
    ) -> Result<(Handle, bool), Win32Error> {
        self.process_io.open(path, readable, writable, disposition)
    }

    fn read_file(&mut self, handle: Handle, length: usize) -> Result<Vec<u8>, Win32Error> {
        self.process_io.read(handle, length)
    }

    fn set_end_of_file(&mut self, handle: Handle) -> Result<(), Win32Error> {
        self.process_io.set_end(handle)
    }

    fn flush_file(&mut self, handle: Handle) -> Result<(), Win32Error> {
        self.process_io.flush(handle)
    }

    fn close_file(&mut self, handle: Handle) -> Result<(), Win32Error> {
        self.process_io.close(handle)
    }

    fn create_directory(&mut self, path: &str) -> Result<(), Win32Error> {
        self.process_io.create_directory(path)
    }

    fn remove_directory(&mut self, path: &str) -> Result<(), Win32Error> {
        self.process_io.remove_directory(path)
    }

    fn remove_file(&mut self, path: &str) -> Result<(), Win32Error> {
        self.process_io.remove_file(path)
    }

    fn file_attributes(&self, path: &str) -> Result<u32, Win32Error> {
        self.process_io.file_attributes(path)
    }

    fn copy_file(
        &mut self,
        source: &str,
        destination: &str,
        fail_if_exists: bool,
    ) -> Result<(), Win32Error> {
        self.process_io
            .copy_file(source, destination, fail_if_exists)
    }

    fn close_kernel_handle(&mut self, handle: Handle) -> Result<(), Win32Error> {
        if self.process_io.contains(handle) {
            self.process_io.close(handle)
        } else {
            self.mutexes
                .close(handle)
                .or_else(|_| self.events.close(handle))
                .or_else(|_| self.tokens.close(handle))
        }
    }

    fn open_process_token(
        &mut self,
        process: Handle,
        desired_access: u32,
    ) -> Result<Handle, Win32Error> {
        if process.0 != u32::MAX {
            return Err(Win32Error::InvalidHandle(process.0));
        }
        self.tokens.open(desired_access)
    }

    fn token_is_open(&self, token: Handle) -> bool {
        self.tokens.contains(token)
    }

    fn create_mutex(
        &mut self,
        name: Option<&str>,
        initial_owner: bool,
    ) -> Result<(Handle, bool), Win32Error> {
        self.mutexes
            .create(name, initial_owner, self.current_thread_id())
    }

    fn release_mutex(&mut self, handle: Handle) -> Result<(), Win32Error> {
        self.mutexes.release(handle, self.current_thread_id())
    }

    fn create_event(
        &mut self,
        name: Option<&str>,
        manual_reset: bool,
        initial_state: bool,
    ) -> Result<(Handle, bool), Win32Error> {
        self.events.create(name, manual_reset, initial_state)
    }

    fn set_event_state(&mut self, handle: Handle, signaled: bool) -> Result<(), Win32Error> {
        self.events.set_state(handle, signaled)
    }

    fn try_wait_for_objects(
        &mut self,
        handles: &[Handle],
        wait_all: bool,
    ) -> Result<Option<u32>, Win32Error> {
        if handles.is_empty() {
            return Err(Win32Error::InvalidArgument("empty wait handle array"));
        }
        let thread = self.current_thread_id();
        let readiness = handles
            .iter()
            .map(|handle| {
                self.events
                    .is_signaled(*handle)
                    .or_else(|| self.mutexes.is_available(*handle, thread))
                    .ok_or(Win32Error::InvalidHandle(handle.0))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if wait_all {
            if !readiness.iter().all(|ready| *ready) {
                return Ok(None);
            }
            for handle in handles {
                self.events
                    .consume(*handle)
                    .or_else(|| self.mutexes.acquire(*handle, thread));
            }
            Ok(Some(0))
        } else if let Some(index) = readiness.iter().position(|ready| *ready) {
            let handle = handles[index];
            self.events
                .consume(handle)
                .or_else(|| self.mutexes.acquire(handle, thread));
            u32::try_from(index)
                .map(Some)
                .map_err(|_| Win32Error::OutOfMemory)
        } else {
            Ok(None)
        }
    }

    fn find_first_file(&mut self, pattern: &str) -> Result<(Handle, FileEntry), Win32Error> {
        self.process_io.find_first(pattern)
    }

    fn find_next_file(&mut self, handle: Handle) -> Result<Option<FileEntry>, Win32Error> {
        self.process_io.find_next(handle)
    }

    fn close_file_search(&mut self, handle: Handle) -> Result<(), Win32Error> {
        self.process_io.close_search(handle)
    }

    fn file_size(&self, handle: Handle) -> Result<u64, Win32Error> {
        self.process_io.file_size(handle)
    }

    fn standard_handle(&self, selector: i32) -> Option<Handle> {
        match selector {
            -10 => Some(Handle(self.standard_handles[0])),
            -11 => Some(Handle(self.standard_handles[1])),
            -12 => Some(Handle(self.standard_handles[2])),
            _ => None,
        }
    }

    fn set_standard_handle(&mut self, selector: i32, handle: Handle) -> Result<bool, Win32Error> {
        let Some((index, offset)) = (match selector {
            -10 => Some((0, 0x18)),
            -11 => Some((1, 0x1c)),
            -12 => Some((2, 0x20)),
            _ => None,
        }) else {
            return Ok(false);
        };
        self.standard_handles[index] = handle.0;
        self.memory
            .write_u32(GuestAddress(self.process_parameters.0 + offset), handle.0)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;
        Ok(true)
    }

    fn write_handle(&mut self, handle: Handle, bytes: &[u8]) -> Result<usize, Win32Error> {
        if self.process_io.contains(handle) {
            return self.process_io.write(handle, bytes);
        }
        if handle.0 == self.standard_handles[1] {
            self.guest_stdout.extend_from_slice(bytes);
        } else if handle.0 == self.standard_handles[2] {
            self.guest_stderr.extend_from_slice(bytes);
        } else {
            return Err(Win32Error::InvalidHandle(handle.0));
        }
        Ok(bytes.len())
    }

    fn seek_file(&mut self, handle: Handle, distance: i64, origin: u32) -> Result<u64, Win32Error> {
        self.process_io.seek(handle, distance, origin)
    }

    fn file_type(&self, handle: Handle) -> Option<u32> {
        if self.process_io.contains(handle) {
            Some(FILE_TYPE_DISK)
        } else if self.standard_handles.contains(&handle.0) {
            Some(FILE_TYPE_CHAR)
        } else {
            None
        }
    }

    fn environment_variable(&self, name: &str) -> Option<&str> {
        self.environment
            .get(&name.to_ascii_uppercase())
            .map(String::as_str)
    }

    fn set_environment_variable(
        &mut self,
        name: &str,
        value: Option<&str>,
    ) -> Result<(), Win32Error> {
        if name.is_empty() || name.contains('=') {
            return Err(Win32Error::InvalidArgument("environment variable name"));
        }
        let key = name.to_ascii_uppercase();
        if let Some(value) = value {
            self.environment.insert(key, value.to_owned());
        } else {
            self.environment.remove(&key);
        }
        let ansi = encode_environment_block_ansi(self.environment);
        let utf16 = encode_environment_block_utf16(self.environment);
        let ansi_address = self.allocate_virtual_memory(
            u32::try_from(ansi.len()).map_err(|_| Win32Error::OutOfMemory)?,
            true,
            true,
            false,
        )?;
        let utf16_address = self.allocate_virtual_memory(
            u32::try_from(utf16.len()).map_err(|_| Win32Error::OutOfMemory)?,
            true,
            true,
            false,
        )?;
        self.write_memory(ansi_address, &ansi)?;
        self.write_memory(utf16_address, &utf16)?;
        *self.environment_block_ansi = ansi_address;
        *self.environment_block_utf16 = utf16_address;
        self.memory
            .write_u32(
                GuestAddress(self.process_parameters.0 + 0x48),
                utf16_address.0,
            )
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;
        Ok(())
    }

    fn environment_block_ansi(&self) -> GuestAddress {
        *self.environment_block_ansi
    }

    fn environment_block_utf16(&self) -> GuestAddress {
        *self.environment_block_utf16
    }

    fn current_directory(&self) -> &str {
        self.current_directory
    }

    fn set_current_directory(&mut self, path: &str) -> Result<(), Win32Error> {
        if path.is_empty() {
            return Err(Win32Error::InvalidArgument("empty current directory"));
        }
        *self.current_directory = path.replace('/', "\\");
        Ok(())
    }

    fn current_process_id(&self) -> u32 {
        GUEST_PROCESS_ID
    }

    fn current_thread_id(&self) -> u32 {
        GUEST_THREAD_ID
    }
}
