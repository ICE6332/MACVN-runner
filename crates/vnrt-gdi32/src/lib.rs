//! Target-driven `gdi32.dll` display and bitmap compatibility surface.

use vnrt_user32::SCREEN_DC_HANDLE;
use vnrt_win32::{
    ApiKey, ApiRegistry, GuestAddress, HostCallContext, HostCallHandler, Win32Error, encode_ansi_z,
    encode_utf16_z,
};

const MODULE: &str = "gdi32.dll";

/// Register the GDI APIs currently required by the selected Guest.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "GetDeviceCaps"), GetDeviceCaps);
    registry.register(ApiKey::new(MODULE, "SelectObject"), SelectObject);
    registry.register(
        ApiKey::new(MODULE, "CreateCompatibleDC"),
        CreateCompatibleDc,
    );
    registry.register(ApiKey::new(MODULE, "DeleteDC"), DeleteDc);
    registry.register(ApiKey::new(MODULE, "DeleteObject"), DeleteObject);
    registry.register(ApiKey::new(MODULE, "GetStockObject"), GetStockObject);
    registry.register(ApiKey::new(MODULE, "GetObjectA"), GetObject { wide: false });
    registry.register(ApiKey::new(MODULE, "GetObjectW"), GetObject { wide: true });
    registry.register(
        ApiKey::new(MODULE, "SetBkMode"),
        SetDcAttribute {
            attribute: 1,
            default: 1,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "SetTextColor"),
        SetDcAttribute {
            attribute: 2,
            default: 0,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "SetBkColor"),
        SetDcAttribute {
            attribute: 3,
            default: 0x00ff_ffff,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "SetROP2"),
        SetDcAttribute {
            attribute: 4,
            default: 13,
        },
    );
    registry.register(
        ApiKey::new(MODULE, "SetStretchBltMode"),
        SetDcAttribute {
            attribute: 5,
            default: 1,
        },
    );
    registry.register(ApiKey::new(MODULE, "BitBlt"), Blit { stretch: false });
    registry.register(ApiKey::new(MODULE, "StretchBlt"), Blit { stretch: true });
    registry.register(ApiKey::new(MODULE, "TextOutA"), TextOut { wide: false });
    registry.register(ApiKey::new(MODULE, "TextOutW"), TextOut { wide: true });
    registry.register(
        ApiKey::new(MODULE, "CreateFontIndirectA"),
        CreateFontIndirect { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "CreateFontIndirectW"),
        CreateFontIndirect { wide: true },
    );
    registry.register(ApiKey::new(MODULE, "CreateDIBSection"), CreateDibSection);
    registry.register(ApiKey::new(MODULE, "CreateBitmap"), CreateBitmap);
    registry.register(
        ApiKey::new(MODULE, "CreateCompatibleBitmap"),
        CreateCompatibleBitmap,
    );
    registry.register(ApiKey::new(MODULE, "ExtCreateRegion"), ExtCreateRegion);
    registry.register(ApiKey::new(MODULE, "CreateRectRgn"), CreateRectRegion);
    registry.register(
        ApiKey::new(MODULE, "CreateRectRgnIndirect"),
        CreateRectRegionIndirect,
    );
    registry.register(ApiKey::new(MODULE, "GetDIBits"), GetDibits { set: false });
    registry.register(ApiKey::new(MODULE, "SetDIBits"), GetDibits { set: true });
    registry.register(
        ApiKey::new(MODULE, "StretchDIBits"),
        TransferDibToDc { stretch: true },
    );
    registry.register(
        ApiKey::new(MODULE, "SetDIBitsToDevice"),
        TransferDibToDc { stretch: false },
    );
    registry.register(
        ApiKey::new(MODULE, "EnumFontFamiliesExA"),
        EnumFontFamilies { wide: false },
    );
    registry.register(
        ApiKey::new(MODULE, "EnumFontFamiliesExW"),
        EnumFontFamilies { wide: true },
    );
}

#[derive(Debug, Clone, Copy)]
struct SelectObject;

#[derive(Debug, Clone, Copy)]
struct CreateCompatibleDc;

#[derive(Debug, Clone, Copy)]
struct DeleteDc;

#[derive(Debug, Clone, Copy)]
struct DeleteObject;

#[derive(Debug, Clone, Copy)]
struct GetStockObject;

#[derive(Debug, Clone, Copy)]
struct GetObject {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct SetDcAttribute {
    attribute: u32,
    default: u32,
}

#[derive(Debug, Clone, Copy)]
struct TextOut {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct Blit {
    stretch: bool,
}

impl HostCallHandler for Blit {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let destination = context.argument_u32(0)?;
        let _destination_x = context.argument_u32(1)?;
        let _destination_y = context.argument_u32(2)?;
        let _destination_width = context.argument_u32(3)?;
        let _destination_height = context.argument_u32(4)?;
        let source = context.argument_u32(5)?;
        let _source_x = context.argument_u32(6)?;
        let _source_y = context.argument_u32(7)?;
        if self.stretch {
            let _source_width = context.argument_u32(8)?;
            let _source_height = context.argument_u32(9)?;
            let _operation = context.argument_u32(10)?;
        } else {
            let _operation = context.argument_u32(8)?;
        }
        let valid = context.is_gdi_dc(destination) && context.is_gdi_dc(source);
        if valid {
            let _ = context.present_selected_bitmap(destination, source)?;
        }
        context.set_return_u32(u32::from(valid));
        context.set_stdcall_cleanup(if self.stretch { 44 } else { 36 });
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct CreateFontIndirect {
    wide: bool,
}

#[derive(Debug, Clone, Copy)]
struct CreateDibSection;

#[derive(Debug, Clone, Copy)]
struct CreateBitmap;

#[derive(Debug, Clone, Copy)]
struct CreateCompatibleBitmap;

#[derive(Debug, Clone, Copy)]
struct ExtCreateRegion;

#[derive(Debug, Clone, Copy)]
struct CreateRectRegion;

#[derive(Debug, Clone, Copy)]
struct CreateRectRegionIndirect;

#[derive(Debug, Clone, Copy)]
struct GetDibits {
    set: bool,
}

#[derive(Debug, Clone, Copy)]
struct TransferDibToDc {
    stretch: bool,
}

#[derive(Debug, Clone, Copy)]
struct EnumFontFamilies {
    wide: bool,
}

impl HostCallHandler for EnumFontFamilies {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let dc = context.argument_u32(0)?;
        let filter = GuestAddress(context.argument_u32(1)?);
        let callback = GuestAddress(context.argument_u32(2)?);
        let parameter = context.argument_u32(3)?;
        let _flags = context.argument_u32(4)?;
        let filter_size = if self.wide { 92 } else { 60 };
        let mut filter_bytes = vec![0; filter_size];
        context.read_memory(filter, &mut filter_bytes)?;
        if !context.is_gdi_dc(dc) || callback.0 == 0 {
            context.set_return_u32(0);
            context.set_stdcall_cleanup(20);
            return Ok(());
        }
        let enum_size = if self.wide { 348 } else { 188 };
        let mut font = vec![0; enum_size];
        font[0..4].copy_from_slice(&(-16_i32).to_le_bytes());
        font[16..20].copy_from_slice(&400_i32.to_le_bytes());
        font[23] = 128; // SHIFTJIS_CHARSET
        font[27] = 0x31; // FIXED_PITCH | FF_MODERN
        if self.wide {
            let name = encode_utf16_z("MS Gothic");
            font[28..28 + name.len()].copy_from_slice(&name);
        } else {
            let name = encode_ansi_z("MS Gothic");
            font[28..28 + name.len()].copy_from_slice(&name);
        }
        let font_address = context.allocate_virtual_memory(enum_size as u32, true, true, false)?;
        context.write_memory(font_address, &font)?;
        let metrics_size = 96_u32;
        let metrics = context.allocate_virtual_memory(metrics_size, true, true, false)?;
        context.write_memory(metrics, &vec![0; metrics_size as usize])?;
        context.request_guest_callback(callback, &[font_address.0, metrics.0, 4, parameter])?;
        context.set_return_u32(1);
        context.set_stdcall_cleanup(20);
        Ok(())
    }
}

impl HostCallHandler for TransferDibToDc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let dc = context.argument_u32(0)?;
        let _destination_x = context.argument_u32(1)?;
        let _destination_y = context.argument_u32(2)?;
        let _destination_width = context.argument_u32(3)?;
        let _destination_height = context.argument_u32(4)?;
        let _source_x = context.argument_u32(5)?;
        let _source_y = context.argument_u32(6)?;
        let (_source_width, source_height, bits_index, info_index, usage_index, cleanup) =
            if self.stretch {
                (
                    context.argument_u32(7)?,
                    context.argument_u32(8)?,
                    9,
                    10,
                    11,
                    52,
                )
            } else {
                let first_scan = context.argument_u32(7)?;
                let scan_lines = context.argument_u32(8)?;
                let _ = first_scan;
                (context.argument_u32(3)?, scan_lines, 9, 10, 11, 48)
            };
        let bits = GuestAddress(context.argument_u32(bits_index)?);
        let info = GuestAddress(context.argument_u32(info_index)?);
        let usage = context.argument_u32(usage_index)?;
        let _operation = if self.stretch {
            context.argument_u32(12)?
        } else {
            0x00cc_0020
        };
        let mut header = [0; 40];
        context.read_memory(info, &mut header)?;
        let width = i32::from_le_bytes(header[4..8].try_into().expect("DIB width"));
        let height = i32::from_le_bytes(header[8..12].try_into().expect("DIB height"));
        let bits_per_pixel = u16::from_le_bytes(header[14..16].try_into().expect("DIB depth"));
        let stride = u64::from(width.unsigned_abs())
            .checked_mul(u64::from(bits_per_pixel))
            .and_then(|row| row.checked_add(31))
            .map(|row| (row / 32) * 4)
            .and_then(|row| u32::try_from(row).ok())
            .ok_or(Win32Error::InvalidArgument("DIB transfer stride"))?;
        let valid = context.is_gdi_dc(dc) && usage <= 1 && width != 0 && height != 0 && bits.0 != 0;
        if valid {
            let _ = context.present_dib(
                dc,
                width.unsigned_abs(),
                height,
                stride,
                bits_per_pixel,
                bits,
            )?;
        }
        let transferred = if valid {
            source_height.min(height.unsigned_abs())
        } else {
            0
        };
        context.set_return_u32(transferred);
        context.set_stdcall_cleanup(cleanup);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct BitmapLayout {
    width: u32,
    height: i32,
    stride: u32,
    planes: u16,
    bits_per_pixel: u16,
    pixels: GuestAddress,
}

fn bitmap_layout(context: &dyn HostCallContext, bitmap: u32) -> Option<BitmapLayout> {
    let descriptor = context.gdi_object(bitmap)?;
    if descriptor.len() < 24 {
        return None;
    }
    let width = u32::from_le_bytes(descriptor[4..8].try_into().ok()?);
    let height = i32::from_le_bytes(descriptor[8..12].try_into().ok()?);
    let stride = u32::from_le_bytes(descriptor[12..16].try_into().ok()?);
    let planes = u16::from_le_bytes(descriptor[16..18].try_into().ok()?);
    let bits_per_pixel = u16::from_le_bytes(descriptor[18..20].try_into().ok()?);
    let pixels = GuestAddress(u32::from_le_bytes(descriptor[20..24].try_into().ok()?));
    (width != 0
        && height != 0
        && stride != 0
        && planes != 0
        && bits_per_pixel != 0
        && pixels.0 != 0)
        .then_some(BitmapLayout {
            width,
            height,
            stride,
            planes,
            bits_per_pixel,
            pixels,
        })
}

impl HostCallHandler for GetDibits {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let dc = context.argument_u32(0)?;
        let bitmap = context.argument_u32(1)?;
        let first_scan = context.argument_u32(2)?;
        let requested_lines = context.argument_u32(3)?;
        let bits = GuestAddress(context.argument_u32(4)?);
        let info = GuestAddress(context.argument_u32(5)?);
        let usage = context.argument_u32(6)?;
        let Some(layout) = bitmap_layout(context, bitmap) else {
            context.set_return_u32(0);
            context.set_stdcall_cleanup(28);
            return Ok(());
        };
        if !context.is_gdi_dc(dc) || usage > 1 || info.0 == 0 {
            context.set_return_u32(0);
            context.set_stdcall_cleanup(28);
            return Ok(());
        }
        let available = layout.height.unsigned_abs().saturating_sub(first_scan);
        let lines = requested_lines.min(available);
        let byte_count = layout
            .stride
            .checked_mul(lines)
            .ok_or(Win32Error::InvalidArgument("DIB scan byte count"))?;
        if bits.0 != 0 && byte_count != 0 {
            let source = GuestAddress(
                layout
                    .pixels
                    .0
                    .checked_add(layout.stride.saturating_mul(first_scan))
                    .ok_or(Win32Error::InvalidArgument("DIB scan offset"))?,
            );
            let mut bytes = vec![0; byte_count as usize];
            if self.set {
                context.read_memory(bits, &mut bytes)?;
                context.write_memory(source, &bytes)?;
            } else {
                context.read_memory(source, &mut bytes)?;
                context.write_memory(bits, &bytes)?;
            }
        }
        if !self.set {
            let mut header = [0; 40];
            header[0..4].copy_from_slice(&40_u32.to_le_bytes());
            header[4..8].copy_from_slice(&layout.width.to_le_bytes());
            header[8..12].copy_from_slice(&layout.height.to_le_bytes());
            header[12..14].copy_from_slice(&layout.planes.to_le_bytes());
            header[14..16].copy_from_slice(&layout.bits_per_pixel.to_le_bytes());
            let full_size = layout
                .stride
                .checked_mul(layout.height.unsigned_abs())
                .ok_or(Win32Error::InvalidArgument("DIB full image size"))?;
            header[20..24].copy_from_slice(&full_size.to_le_bytes());
            context.write_memory(info, &header)?;
        }
        context.set_return_u32(lines);
        context.set_stdcall_cleanup(28);
        Ok(())
    }
}

fn rectangular_region_descriptor(bounds: &[u8; 16]) -> Vec<u8> {
    let mut region = vec![0; 48];
    region[0..4].copy_from_slice(&32_u32.to_le_bytes());
    region[4..8].copy_from_slice(&1_u32.to_le_bytes()); // RDH_RECTANGLES
    region[8..12].copy_from_slice(&1_u32.to_le_bytes());
    region[12..16].copy_from_slice(&16_u32.to_le_bytes());
    region[16..32].copy_from_slice(bounds);
    region[32..48].copy_from_slice(bounds);
    region
}

impl HostCallHandler for ExtCreateRegion {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let transform = GuestAddress(context.argument_u32(0)?);
        let byte_count = context.argument_u32(1)? as usize;
        let data = GuestAddress(context.argument_u32(2)?);
        if transform.0 != 0 {
            let mut xform = [0; 24];
            context.read_memory(transform, &mut xform)?;
        }
        if !(32..=16 * 1024 * 1024).contains(&byte_count) {
            context.set_return_u32(0);
            context.set_stdcall_cleanup(12);
            return Ok(());
        }
        let mut region = vec![0; byte_count];
        context.read_memory(data, &mut region)?;
        let header_size = u32::from_le_bytes(region[0..4].try_into().expect("region header size"));
        let region_type = u32::from_le_bytes(region[4..8].try_into().expect("region type"));
        if header_size < 32 || region_type != 1 {
            context.set_return_u32(0);
        } else {
            let handle = context.create_gdi_object(&region);
            context.set_return_u32(handle);
        }
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for CreateRectRegion {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let coordinates = [
            context.argument_u32(0)?,
            context.argument_u32(1)?,
            context.argument_u32(2)?,
            context.argument_u32(3)?,
        ];
        let mut bounds = [0; 16];
        for (target, value) in bounds.chunks_exact_mut(4).zip(coordinates) {
            target.copy_from_slice(&value.to_le_bytes());
        }
        let region = context.create_gdi_object(&rectangular_region_descriptor(&bounds));
        context.set_return_u32(region);
        context.set_stdcall_cleanup(16);
        Ok(())
    }
}

impl HostCallHandler for CreateRectRegionIndirect {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let input = GuestAddress(context.argument_u32(0)?);
        let mut bounds = [0; 16];
        context.read_memory(input, &mut bounds)?;
        let region = context.create_gdi_object(&rectangular_region_descriptor(&bounds));
        context.set_return_u32(region);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

fn allocate_bitmap(
    context: &mut dyn HostCallContext,
    width: u32,
    height: u32,
    planes: u16,
    bits_per_pixel: u16,
    input: GuestAddress,
) -> Result<u32, Win32Error> {
    let row_bits = u64::from(width)
        .checked_mul(u64::from(planes))
        .and_then(|bits| bits.checked_mul(u64::from(bits_per_pixel)))
        .ok_or(Win32Error::InvalidArgument("bitmap row overflow"))?;
    let stride = row_bits
        .checked_add(15)
        .map(|bits| (bits / 16) * 2)
        .and_then(|bytes| u32::try_from(bytes).ok())
        .ok_or(Win32Error::InvalidArgument("bitmap stride overflow"))?;
    let image_size = stride
        .checked_mul(height)
        .filter(|size| *size != 0 && *size <= 256 * 1024 * 1024)
        .ok_or(Win32Error::InvalidArgument("bitmap image size"))?;
    let pixels = context.allocate_virtual_memory(image_size, true, true, false)?;
    if input.0 != 0 {
        let mut bytes = vec![0; image_size as usize];
        context.read_memory(input, &mut bytes)?;
        context.write_memory(pixels, &bytes)?;
    }
    let mut descriptor = vec![0; 24];
    descriptor[4..8].copy_from_slice(&width.to_le_bytes());
    descriptor[8..12].copy_from_slice(&height.to_le_bytes());
    descriptor[12..16].copy_from_slice(&stride.to_le_bytes());
    descriptor[16..18].copy_from_slice(&planes.to_le_bytes());
    descriptor[18..20].copy_from_slice(&bits_per_pixel.to_le_bytes());
    descriptor[20..24].copy_from_slice(&pixels.0.to_le_bytes());
    Ok(context.create_gdi_object(&descriptor))
}

impl HostCallHandler for CreateBitmap {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let width = context.argument_u32(0)?;
        let height = context.argument_u32(1)?;
        let planes = context.argument_u32(2)? as u16;
        let bits_per_pixel = context.argument_u32(3)? as u16;
        let input = GuestAddress(context.argument_u32(4)?);
        let bitmap = if width == 0 || height == 0 || planes == 0 || bits_per_pixel == 0 {
            0
        } else {
            allocate_bitmap(context, width, height, planes, bits_per_pixel, input)?
        };
        context.set_return_u32(bitmap);
        context.set_stdcall_cleanup(20);
        Ok(())
    }
}

impl HostCallHandler for CreateCompatibleBitmap {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let dc = context.argument_u32(0)?;
        let width = context.argument_u32(1)?.max(1);
        let height = context.argument_u32(2)?.max(1);
        let bitmap = if context.is_gdi_dc(dc) {
            allocate_bitmap(context, width, height, 1, 32, GuestAddress(0))?
        } else {
            0
        };
        context.set_return_u32(bitmap);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

impl HostCallHandler for CreateDibSection {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let dc = context.argument_u32(0)?;
        let info = GuestAddress(context.argument_u32(1)?);
        let usage = context.argument_u32(2)?;
        let output_bits = GuestAddress(context.argument_u32(3)?);
        let section = context.argument_u32(4)?;
        let offset = context.argument_u32(5)?;
        if (dc != 0 && !context.is_gdi_dc(dc)) || usage > 1 || section != 0 || offset != 0 {
            context.set_last_error(87); // ERROR_INVALID_PARAMETER
            context.set_return_u32(0);
            context.set_stdcall_cleanup(24);
            return Ok(());
        }
        let mut header = [0; 40];
        context.read_memory(info, &mut header)?;
        let header_size = u32::from_le_bytes(header[0..4].try_into().expect("biSize"));
        let width = i32::from_le_bytes(header[4..8].try_into().expect("biWidth"));
        let height = i32::from_le_bytes(header[8..12].try_into().expect("biHeight"));
        let planes = u16::from_le_bytes(header[12..14].try_into().expect("biPlanes"));
        let bits_per_pixel = u16::from_le_bytes(header[14..16].try_into().expect("biBitCount"));
        let compression = u32::from_le_bytes(header[16..20].try_into().expect("biCompression"));
        let supported = header_size >= 40
            && width > 0
            && height != 0
            && planes == 1
            && matches!(bits_per_pixel, 1 | 4 | 8 | 16 | 24 | 32)
            && matches!(compression, 0 | 3);
        if !supported {
            context.set_last_error(87); // ERROR_INVALID_PARAMETER
            context.set_return_u32(0);
            context.set_stdcall_cleanup(24);
            return Ok(());
        }
        let width = width as u32;
        let rows = height.unsigned_abs();
        let stride = u64::from(width)
            .checked_mul(u64::from(bits_per_pixel))
            .and_then(|bits| bits.checked_add(31))
            .map(|bits| (bits / 32) * 4)
            .and_then(|bytes| u32::try_from(bytes).ok())
            .ok_or(Win32Error::InvalidArgument("DIB stride overflow"))?;
        let image_size = stride
            .checked_mul(rows)
            .filter(|size| *size != 0 && *size <= 256 * 1024 * 1024)
            .ok_or(Win32Error::InvalidArgument("DIB image size"))?;
        let pixels = context.allocate_virtual_memory(image_size, true, true, false)?;
        context.write_memory(output_bits, &pixels.0.to_le_bytes())?;

        // DIBSECTION = BITMAP (24) + BITMAPINFOHEADER (40) + masks/section/offset (20).
        let mut descriptor = vec![0; 84];
        descriptor[4..8].copy_from_slice(&width.to_le_bytes());
        descriptor[8..12].copy_from_slice(&height.to_le_bytes());
        descriptor[12..16].copy_from_slice(&stride.to_le_bytes());
        descriptor[16..18].copy_from_slice(&planes.to_le_bytes());
        descriptor[18..20].copy_from_slice(&bits_per_pixel.to_le_bytes());
        descriptor[20..24].copy_from_slice(&pixels.0.to_le_bytes());
        descriptor[24..64].copy_from_slice(&header);
        descriptor[44..48].copy_from_slice(&image_size.to_le_bytes());
        if compression == 3 {
            let mut masks = [0; 12];
            context.read_memory(GuestAddress(info.0 + 40), &mut masks)?;
            descriptor[64..76].copy_from_slice(&masks);
        }
        let bitmap = context.create_gdi_object(&descriptor);
        context.set_last_error(0);
        context.set_return_u32(bitmap);
        context.set_stdcall_cleanup(24);
        Ok(())
    }
}

impl HostCallHandler for CreateFontIndirect {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let input = GuestAddress(context.argument_u32(0)?);
        let mut descriptor = vec![0; if self.wide { 92 } else { 60 }];
        context.read_memory(input, &mut descriptor)?;
        let font = context.create_gdi_object(&descriptor);
        context.set_return_u32(font);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for TextOut {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let dc = context.argument_u32(0)?;
        let _x = context.argument_u32(1)?;
        let _y = context.argument_u32(2)?;
        let text = GuestAddress(context.argument_u32(3)?);
        let count = context.argument_u32(4)? as usize;
        let byte_count = count
            .checked_mul(if self.wide { 2 } else { 1 })
            .ok_or(Win32Error::InvalidArgument("TextOut character count"))?;
        let mut bytes = vec![0; byte_count];
        context.read_memory(text, &mut bytes)?;
        context.set_return_u32(u32::from(context.is_gdi_dc(dc)));
        context.set_stdcall_cleanup(20);
        Ok(())
    }
}

impl HostCallHandler for SetDcAttribute {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let dc = context.argument_u32(0)?;
        let value = context.argument_u32(1)?;
        let previous = context
            .replace_gdi_dc_attribute(dc, self.attribute, value, self.default)
            .unwrap_or(0);
        context.set_return_u32(previous);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for GetObject {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let object = context.argument_u32(0)?;
        let capacity = context.argument_u32(1)? as usize;
        let output = GuestAddress(context.argument_u32(2)?);
        let descriptor = context
            .gdi_object(object)
            .or_else(|| stock_object_descriptor(object, self.wide));
        let Some(descriptor) = descriptor else {
            context.set_return_u32(0);
            context.set_stdcall_cleanup(12);
            return Ok(());
        };
        if capacity != 0 && output.0 != 0 {
            context.write_memory(output, &descriptor[..descriptor.len().min(capacity)])?;
        }
        context.set_return_u32(descriptor.len() as u32);
        context.set_stdcall_cleanup(12);
        Ok(())
    }
}

fn stock_object_descriptor(object: u32, wide: bool) -> Option<Vec<u8>> {
    if object & 0xffff_0000 != 0x000c_0000 {
        return None;
    }
    let index = object & 0xffff;
    match index {
        0..=5 | 18 => Some(vec![0; 12]), // LOGBRUSH
        6..=8 | 19 => Some(vec![0; 16]), // LOGPEN
        10..=14 | 16..=17 => Some(vec![0; if wide { 92 } else { 60 }]), // LOGFONT
        15 => Some(vec![0; 4]),          // palette entry count
        _ => None,
    }
}

impl HostCallHandler for SelectObject {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let dc = context.argument_u32(0)?;
        let object = context.argument_u32(1)?;
        let previous = context.select_gdi_object(dc, object).unwrap_or(0);
        context.set_return_u32(previous);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

impl HostCallHandler for CreateCompatibleDc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let source = context.argument_u32(0)?;
        let dc = context.create_memory_dc(source).unwrap_or(0);
        context.set_return_u32(dc);
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for DeleteDc {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let dc = context.argument_u32(0)?;
        let deleted = context.delete_memory_dc(dc);
        context.set_return_u32(u32::from(deleted));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for DeleteObject {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let object = context.argument_u32(0)?;
        let stock = object & 0xffff_0000 == 0x000c_0000;
        let deleted = stock || context.delete_gdi_object(object);
        context.set_return_u32(u32::from(deleted));
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for GetStockObject {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let index = context.argument_u32(0)?;
        context.set_return_u32(if index <= 20 { 0x000c_0000 | index } else { 0 });
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct GetDeviceCaps;

impl HostCallHandler for GetDeviceCaps {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let dc = context.argument_u32(0)?;
        if dc != SCREEN_DC_HANDLE && !context.is_window_dc(dc) {
            context.set_return_u32(0);
            context.set_stdcall_cleanup(8);
            return Ok(());
        }
        let (display_width, display_height) = context.primary_display_size();
        let value = match context.argument_u32(1)? {
            0 => 0x0400,                // DRIVERVERSION
            2 => 1,                     // TECHNOLOGY = DT_RASDISPLAY
            4 => 338,                   // HORZSIZE, millimeters
            6 => 190,                   // VERTSIZE
            8 | 118 => display_width,   // HORZRES / DESKTOPHORZRES
            10 | 117 => display_height, // VERTRES / DESKTOPVERTRES
            12 => 32,                   // BITSPIXEL
            14 => 1,                    // PLANES
            24 => u32::MAX,             // NUMCOLORS for a true-color display
            88 | 90 => 96,              // LOGPIXELSX / LOGPIXELSY
            108 => 24,                  // COLORRES
            110 => display_width,       // PHYSICALWIDTH
            111 => display_height,      // PHYSICALHEIGHT
            112 | 113 => 0,             // PHYSICALOFFSETX / PHYSICALOFFSETY
            116 => 60,                  // VREFRESH
            _ => 0,
        };
        context.set_return_u32(value);
        context.set_stdcall_cleanup(8);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_display_capability_query() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 31);
    }
}
