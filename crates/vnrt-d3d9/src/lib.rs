//! Target-driven `d3d9.dll` COM surface for the first-frame path.
//!
//! Builds Guest COM objects whose vtables point at Host thunks. Method bodies
//! are filled as the selected game reaches them; discovery-safe stubs return
//! `D3D_OK` with zeroed outputs.

use tracing::debug;
use vnrt_win32::{
    ApiKey, ApiRegistry, GuestAddress, HostCallContext, HostCallHandler, TextureDescriptor,
    TextureFormat, TextureId, Win32Error,
};

const MODULE: &str = "d3d9.dll";
const D3D_OK: u32 = 0;
const E_INVALIDARG: u32 = 0x8007_0057;
const D3DDEVTYPE_HAL: u32 = 1;
const D3DADAPTER_DEFAULT: u32 = 0;
const KIND_D3D9: u32 = 1;
const KIND_DEVICE: u32 = 2;
const KIND_TEXTURE: u32 = 3;
const KIND_SURFACE: u32 = 4;

/// Register Direct3D 9 factory and COM method Host thunks.
pub fn register(registry: &mut ApiRegistry) {
    registry.register(ApiKey::new(MODULE, "Direct3DCreate9"), Direct3DCreate9);
    for &(name, method) in METHODS {
        registry.register(ApiKey::new(MODULE, name), method);
    }
}

const METHODS: &[(&str, Method)] = &[
    ("IUnknown_QueryInterface", Method::QueryInterface),
    ("IUnknown_AddRef", Method::AddRef),
    ("IUnknown_Release", Method::Release),
    ("IDirect3D9_GetAdapterCount", Method::GetAdapterCount),
    (
        "IDirect3D9_GetAdapterDisplayMode",
        Method::GetAdapterDisplayMode,
    ),
    ("IDirect3D9_GetDeviceCaps", Method::GetDeviceCaps9),
    ("IDirect3D9_CheckDeviceType", Method::CheckOk { cleanup: 20 }),
    (
        "IDirect3D9_CheckDeviceFormat",
        Method::CheckOk { cleanup: 28 },
    ),
    (
        "IDirect3D9_CheckDeviceMultiSampleType",
        Method::CheckOk { cleanup: 28 },
    ),
    (
        "IDirect3D9_CheckDepthStencilMatch",
        Method::CheckOk { cleanup: 24 },
    ),
    ("IDirect3D9_CreateDevice", Method::CreateDevice),
    ("IDirect3D9_StubOk0", Method::StubOk { cleanup: 4 }),
    ("IDirect3D9_StubOk4", Method::StubOk { cleanup: 8 }),
    ("IDirect3D9_StubOk8", Method::StubOk { cleanup: 12 }),
    ("IDirect3D9_StubOk12", Method::StubOk { cleanup: 16 }),
    ("IDirect3D9_StubOk16", Method::StubOk { cleanup: 20 }),
    ("IDirect3D9_StubOk20", Method::StubOk { cleanup: 24 }),
    ("IDirect3D9_StubOk24", Method::StubOk { cleanup: 28 }),
    (
        "IDirect3DDevice9_TestCooperativeLevel",
        Method::StubOk { cleanup: 4 },
    ),
    (
        "IDirect3DDevice9_GetAvailableTextureMem",
        Method::GetAvailableTextureMem,
    ),
    ("IDirect3DDevice9_GetDirect3D", Method::GetDirect3D),
    (
        "IDirect3DDevice9_GetDeviceCaps",
        Method::GetDeviceCapsDevice,
    ),
    ("IDirect3DDevice9_GetDisplayMode", Method::GetDisplayMode),
    (
        "IDirect3DDevice9_GetCreationParameters",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_SetCursorProperties",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_SetCursorPosition",
        Method::StubOk { cleanup: 12 },
    ),
    ("IDirect3DDevice9_ShowCursor", Method::ShowCursor),
    (
        "IDirect3DDevice9_CreateAdditionalSwapChain",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetSwapChain",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetNumberOfSwapChains",
        Method::GetNumberOfSwapChains,
    ),
    ("IDirect3DDevice9_Reset", Method::StubOk { cleanup: 8 }),
    ("IDirect3DDevice9_Present", Method::Present),
    ("IDirect3DDevice9_GetBackBuffer", Method::GetBackBuffer),
    (
        "IDirect3DDevice9_GetRasterStatus",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_SetDialogBoxMode",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_SetGammaRamp",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetGammaRamp",
        Method::StubOk { cleanup: 8 },
    ),
    ("IDirect3DDevice9_CreateTexture", Method::CreateTexture),
    (
        "IDirect3DDevice9_CreateVolumeTexture",
        Method::StubOk { cleanup: 32 },
    ),
    (
        "IDirect3DDevice9_CreateCubeTexture",
        Method::StubOk { cleanup: 28 },
    ),
    (
        "IDirect3DDevice9_CreateVertexBuffer",
        Method::CreateVertexBuffer,
    ),
    (
        "IDirect3DDevice9_CreateIndexBuffer",
        Method::CreateIndexBuffer,
    ),
    (
        "IDirect3DDevice9_CreateRenderTarget",
        Method::StubOk { cleanup: 32 },
    ),
    (
        "IDirect3DDevice9_CreateDepthStencilSurface",
        Method::StubOk { cleanup: 32 },
    ),
    (
        "IDirect3DDevice9_UpdateSurface",
        Method::StubOk { cleanup: 20 },
    ),
    (
        "IDirect3DDevice9_UpdateTexture",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetRenderTargetData",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetFrontBufferData",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_StretchRect",
        Method::StubOk { cleanup: 24 },
    ),
    (
        "IDirect3DDevice9_ColorFill",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_CreateOffscreenPlainSurface",
        Method::StubOk { cleanup: 28 },
    ),
    (
        "IDirect3DDevice9_SetRenderTarget",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetRenderTarget",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_SetDepthStencilSurface",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetDepthStencilSurface",
        Method::StubOk { cleanup: 8 },
    ),
    ("IDirect3DDevice9_BeginScene", Method::StubOk { cleanup: 4 }),
    ("IDirect3DDevice9_EndScene", Method::StubOk { cleanup: 4 }),
    ("IDirect3DDevice9_Clear", Method::StubOk { cleanup: 28 }),
    (
        "IDirect3DDevice9_SetTransform",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetTransform",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_MultiplyTransform",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_SetViewport",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetViewport",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_SetMaterial",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetMaterial",
        Method::StubOk { cleanup: 8 },
    ),
    ("IDirect3DDevice9_SetLight", Method::StubOk { cleanup: 12 }),
    ("IDirect3DDevice9_GetLight", Method::StubOk { cleanup: 12 }),
    (
        "IDirect3DDevice9_LightEnable",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetLightEnable",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_SetClipPlane",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetClipPlane",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_SetRenderState",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetRenderState",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_CreateStateBlock",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_BeginStateBlock",
        Method::StubOk { cleanup: 4 },
    ),
    (
        "IDirect3DDevice9_EndStateBlock",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_SetClipStatus",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetClipStatus",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetTexture",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_SetTexture",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetTextureStageState",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_SetTextureStageState",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_GetSamplerState",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_SetSamplerState",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_ValidateDevice",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_SetPaletteEntries",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetPaletteEntries",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_SetCurrentTexturePalette",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetCurrentTexturePalette",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_SetScissorRect",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetScissorRect",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_SetSoftwareVertexProcessing",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetSoftwareVertexProcessing",
        Method::ReturnZero { cleanup: 4 },
    ),
    (
        "IDirect3DDevice9_SetNPatchMode",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetNPatchMode",
        Method::ReturnZero { cleanup: 4 },
    ),
    (
        "IDirect3DDevice9_DrawPrimitive",
        Method::StubOk { cleanup: 20 },
    ),
    (
        "IDirect3DDevice9_DrawIndexedPrimitive",
        Method::StubOk { cleanup: 32 },
    ),
    (
        "IDirect3DDevice9_DrawPrimitiveUP",
        Method::StubOk { cleanup: 20 },
    ),
    (
        "IDirect3DDevice9_DrawIndexedPrimitiveUP",
        Method::StubOk { cleanup: 36 },
    ),
    (
        "IDirect3DDevice9_ProcessVertices",
        Method::StubOk { cleanup: 28 },
    ),
    (
        "IDirect3DDevice9_CreateVertexDeclaration",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_SetVertexDeclaration",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetVertexDeclaration",
        Method::StubOk { cleanup: 8 },
    ),
    ("IDirect3DDevice9_SetFVF", Method::StubOk { cleanup: 8 }),
    ("IDirect3DDevice9_GetFVF", Method::StubOk { cleanup: 8 }),
    (
        "IDirect3DDevice9_CreateVertexShader",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_SetVertexShader",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetVertexShader",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_SetVertexShaderConstantF",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_GetVertexShaderConstantF",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_SetVertexShaderConstantI",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_GetVertexShaderConstantI",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_SetVertexShaderConstantB",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_GetVertexShaderConstantB",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_SetStreamSource",
        Method::StubOk { cleanup: 20 },
    ),
    (
        "IDirect3DDevice9_GetStreamSource",
        Method::StubOk { cleanup: 20 },
    ),
    (
        "IDirect3DDevice9_SetStreamSourceFreq",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_GetStreamSourceFreq",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_SetIndices",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetIndices",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_CreatePixelShader",
        Method::StubOk { cleanup: 12 },
    ),
    (
        "IDirect3DDevice9_SetPixelShader",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_GetPixelShader",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_SetPixelShaderConstantF",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_GetPixelShaderConstantF",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_SetPixelShaderConstantI",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_GetPixelShaderConstantI",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_SetPixelShaderConstantB",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_GetPixelShaderConstantB",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_DrawRectPatch",
        Method::StubOk { cleanup: 20 },
    ),
    (
        "IDirect3DDevice9_DrawTriPatch",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DDevice9_DeletePatch",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DDevice9_CreateQuery",
        Method::StubOk { cleanup: 12 },
    ),
    ("IDirect3DDevice9_StubOk0", Method::StubOk { cleanup: 4 }),
    ("IDirect3DTexture9_GetLevelCount", Method::GetLevelCount),
    ("IDirect3DTexture9_GetLevelDesc", Method::GetLevelDesc),
    (
        "IDirect3DTexture9_GetSurfaceLevel",
        Method::GetSurfaceLevel,
    ),
    ("IDirect3DTexture9_LockRect", Method::LockRect),
    ("IDirect3DTexture9_UnlockRect", Method::UnlockRect),
    (
        "IDirect3DTexture9_AddDirtyRect",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DBaseTexture9_SetLOD",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DBaseTexture9_GetLOD",
        Method::ReturnZero { cleanup: 4 },
    ),
    (
        "IDirect3DBaseTexture9_SetAutoGenFilterType",
        Method::StubOk { cleanup: 8 },
    ),
    (
        "IDirect3DBaseTexture9_GetAutoGenFilterType",
        Method::GetAutoGenFilterType,
    ),
    (
        "IDirect3DBaseTexture9_GenerateMipSubLevels",
        Method::StubOk { cleanup: 4 },
    ),
    (
        "IDirect3DResource9_GetDevice",
        Method::GetDeviceFromResource,
    ),
    (
        "IDirect3DResource9_SetPrivateData",
        Method::StubOk { cleanup: 20 },
    ),
    (
        "IDirect3DResource9_GetPrivateData",
        Method::StubOk { cleanup: 16 },
    ),
    (
        "IDirect3DResource9_FreePrivateData",
        Method::StubOk { cleanup: 8 },
    ),
    ("IDirect3DResource9_SetPriority", Method::SetPriority),
    (
        "IDirect3DResource9_GetPriority",
        Method::ReturnZero { cleanup: 4 },
    ),
    ("IDirect3DResource9_PreLoad", Method::StubOk { cleanup: 4 }),
    ("IDirect3DResource9_GetType", Method::GetResourceType),
    (
        "IDirect3DSurface9_GetContainer",
        Method::StubOk { cleanup: 12 },
    ),
    ("IDirect3DSurface9_GetDesc", Method::GetSurfaceDesc),
    ("IDirect3DSurface9_LockRect", Method::LockRect),
    ("IDirect3DSurface9_UnlockRect", Method::UnlockRect),
    ("IDirect3DVertexBuffer9_Lock", Method::LockBuffer),
    (
        "IDirect3DVertexBuffer9_Unlock",
        Method::StubOk { cleanup: 4 },
    ),
    (
        "IDirect3DVertexBuffer9_GetDesc",
        Method::StubOk { cleanup: 8 },
    ),
    ("IDirect3DIndexBuffer9_Lock", Method::LockBuffer),
    (
        "IDirect3DIndexBuffer9_Unlock",
        Method::StubOk { cleanup: 4 },
    ),
    (
        "IDirect3DIndexBuffer9_GetDesc",
        Method::StubOk { cleanup: 8 },
    ),
];

#[derive(Debug, Clone, Copy)]
struct Direct3DCreate9;

#[derive(Debug, Clone, Copy)]
enum Method {
    QueryInterface,
    AddRef,
    Release,
    GetAdapterCount,
    GetAdapterDisplayMode,
    GetDeviceCaps9,
    GetDeviceCapsDevice,
    CreateDevice,
    GetAvailableTextureMem,
    GetDirect3D,
    GetDisplayMode,
    ShowCursor,
    GetNumberOfSwapChains,
    Present,
    GetBackBuffer,
    CreateTexture,
    CreateVertexBuffer,
    CreateIndexBuffer,
    GetLevelCount,
    GetLevelDesc,
    GetSurfaceLevel,
    LockRect,
    UnlockRect,
    GetAutoGenFilterType,
    GetDeviceFromResource,
    SetPriority,
    GetResourceType,
    GetSurfaceDesc,
    LockBuffer,
    CheckOk { cleanup: u32 },
    StubOk { cleanup: u32 },
    ReturnZero { cleanup: u32 },
}

impl HostCallHandler for Direct3DCreate9 {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let sdk_version = context.argument_u32(0)?;
        debug!(sdk_version, "Direct3DCreate9");
        match create_d3d9(context) {
            Ok(object) => {
                context.set_return_u32(object.0);
                context.set_last_error(0);
            }
            Err(Win32Error::OutOfMemory) => {
                context.set_return_u32(0);
                context.set_last_error(8);
            }
            Err(error) => return Err(error),
        }
        context.set_stdcall_cleanup(4);
        Ok(())
    }
}

impl HostCallHandler for Method {
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        let this = GuestAddress(context.argument_u32(0)?);
        match *self {
            Self::QueryInterface => {
                let value = query_interface(context, this)?;
                finish(context, value, 12);
            }
            Self::AddRef => {
                let value = add_ref(context, this)?;
                finish(context, value, 4);
            }
            Self::Release => {
                let value = release(context, this)?;
                finish(context, value, 4);
            }
            Self::GetAdapterCount => finish(context, 1, 4),
            Self::GetAdapterDisplayMode => {
                let value = get_adapter_display_mode(context)?;
                finish(context, value, 12);
            }
            Self::GetDeviceCaps9 => {
                let value = get_device_caps(context, true)?;
                finish(context, value, 12);
            }
            Self::GetDeviceCapsDevice => {
                let value = get_device_caps(context, false)?;
                finish(context, value, 8);
            }
            Self::CreateDevice => {
                let value = create_device(context, this)?;
                finish(context, value, 28);
            }
            Self::GetAvailableTextureMem => finish(context, 512 * 1024 * 1024, 4),
            Self::GetDirect3D => {
                let value = get_parent_d3d(context, this)?;
                finish(context, value, 8);
            }
            Self::GetDisplayMode => {
                let value = get_display_mode_arg(context)?;
                finish(context, value, 12);
            }
            Self::ShowCursor => {
                let show = context.argument_u32(1)? != 0;
                let visible = context.adjust_cursor_display_count(show) >= 0;
                finish(context, u32::from(visible), 8);
            }
            Self::GetNumberOfSwapChains => finish(context, 1, 4),
            Self::Present => finish(context, D3D_OK, 20),
            Self::GetBackBuffer => {
                let value = get_back_buffer(context, this)?;
                finish(context, value, 20);
            }
            Self::CreateTexture => {
                let value = create_texture(context, this)?;
                finish(context, value, 32);
            }
            Self::CreateVertexBuffer => {
                let value = create_buffer(context, this, false)?;
                finish(context, value, 28);
            }
            Self::CreateIndexBuffer => {
                let value = create_buffer(context, this, true)?;
                finish(context, value, 28);
            }
            Self::GetLevelCount => finish(context, 1, 4),
            Self::GetLevelDesc => {
                let value = get_level_desc(context, this)?;
                finish(context, value, 12);
            }
            Self::GetSurfaceLevel => {
                let value = get_surface_level(context, this)?;
                finish(context, value, 12);
            }
            Self::LockRect => {
                let value = lock_rect(context, this)?;
                finish(context, value, 20);
            }
            Self::UnlockRect => {
                let value = unlock_rect(context, this)?;
                finish(context, value, 8);
            }
            Self::GetAutoGenFilterType => finish(context, 1, 4),
            Self::GetDeviceFromResource => {
                let value = get_device_from_resource(context, this)?;
                finish(context, value, 8);
            }
            Self::SetPriority => {
                let priority = context.argument_u32(1)?;
                finish(context, priority, 8);
            }
            Self::GetResourceType => {
                let kind = object_kind(context, this).unwrap_or(0);
                let ty = match kind {
                    KIND_TEXTURE => 3,
                    KIND_SURFACE => 1,
                    7 => 7,
                    _ => 6,
                };
                finish(context, ty, 4);
            }
            Self::GetSurfaceDesc => {
                let value = get_surface_desc(context, this)?;
                finish(context, value, 8);
            }
            Self::LockBuffer => {
                let value = lock_buffer(context, this)?;
                finish(context, value, 20);
            }
            Self::CheckOk { cleanup } | Self::StubOk { cleanup } => {
                finish(context, D3D_OK, cleanup);
            }
            Self::ReturnZero { cleanup } => finish(context, 0, cleanup),
        }
        Ok(())
    }
}

fn finish(context: &mut dyn HostCallContext, value: u32, cleanup: u32) {
    context.set_return_u32(value);
    context.set_stdcall_cleanup(cleanup);
}

fn create_d3d9(context: &mut dyn HostCallContext) -> Result<GuestAddress, Win32Error> {
    let methods = [
        "IUnknown_QueryInterface",
        "IUnknown_AddRef",
        "IUnknown_Release",
        "IDirect3D9_StubOk4",
        "IDirect3D9_GetAdapterCount",
        "IDirect3D9_StubOk12",
        "IDirect3D9_StubOk8",
        "IDirect3D9_StubOk12",
        "IDirect3D9_GetAdapterDisplayMode",
        "IDirect3D9_CheckDeviceType",
        "IDirect3D9_CheckDeviceFormat",
        "IDirect3D9_CheckDeviceMultiSampleType",
        "IDirect3D9_CheckDepthStencilMatch",
        "IDirect3D9_StubOk20",
        "IDirect3D9_GetDeviceCaps",
        "IDirect3D9_StubOk4",
        "IDirect3D9_CreateDevice",
    ];
    create_com_object(context, &methods, KIND_D3D9, 16)
}

fn create_device(
    context: &mut dyn HostCallContext,
    d3d: GuestAddress,
) -> Result<u32, Win32Error> {
    let adapter = context.argument_u32(1)?;
    let device_type = context.argument_u32(2)?;
    let focus_window = context.argument_u32(3)?;
    let _behavior = context.argument_u32(4)?;
    let params = GuestAddress(context.argument_u32(5)?);
    let out = GuestAddress(context.argument_u32(6)?);
    if adapter != D3DADAPTER_DEFAULT || device_type != D3DDEVTYPE_HAL || out.0 == 0 {
        return Ok(E_INVALIDARG);
    }
    let (width, height) = read_present_size(context, params)?;
    debug!(focus_window, width, height, "IDirect3D9::CreateDevice");
    let methods = device_vtable_names();
    let device = create_com_object(context, &methods, KIND_DEVICE, 32)?;
    write_u32(context, GuestAddress(device.0 + 12), d3d.0)?;
    write_u32(context, GuestAddress(device.0 + 16), focus_window)?;
    write_u32(context, GuestAddress(device.0 + 20), width)?;
    write_u32(context, GuestAddress(device.0 + 24), height)?;
    let _ = add_ref(context, d3d)?;
    write_u32(context, out, device.0)?;
    Ok(D3D_OK)
}

fn device_vtable_names() -> [&'static str; 119] {
    [
        "IUnknown_QueryInterface",
        "IUnknown_AddRef",
        "IUnknown_Release",
        "IDirect3DDevice9_TestCooperativeLevel",
        "IDirect3DDevice9_GetAvailableTextureMem",
        "IDirect3DDevice9_StubOk0",
        "IDirect3DDevice9_GetDirect3D",
        "IDirect3DDevice9_GetDeviceCaps",
        "IDirect3DDevice9_GetDisplayMode",
        "IDirect3DDevice9_GetCreationParameters",
        "IDirect3DDevice9_SetCursorProperties",
        "IDirect3DDevice9_SetCursorPosition",
        "IDirect3DDevice9_ShowCursor",
        "IDirect3DDevice9_CreateAdditionalSwapChain",
        "IDirect3DDevice9_GetSwapChain",
        "IDirect3DDevice9_GetNumberOfSwapChains",
        "IDirect3DDevice9_Reset",
        "IDirect3DDevice9_Present",
        "IDirect3DDevice9_GetBackBuffer",
        "IDirect3DDevice9_GetRasterStatus",
        "IDirect3DDevice9_SetDialogBoxMode",
        "IDirect3DDevice9_SetGammaRamp",
        "IDirect3DDevice9_GetGammaRamp",
        "IDirect3DDevice9_CreateTexture",
        "IDirect3DDevice9_CreateVolumeTexture",
        "IDirect3DDevice9_CreateCubeTexture",
        "IDirect3DDevice9_CreateVertexBuffer",
        "IDirect3DDevice9_CreateIndexBuffer",
        "IDirect3DDevice9_CreateRenderTarget",
        "IDirect3DDevice9_CreateDepthStencilSurface",
        "IDirect3DDevice9_UpdateSurface",
        "IDirect3DDevice9_UpdateTexture",
        "IDirect3DDevice9_GetRenderTargetData",
        "IDirect3DDevice9_GetFrontBufferData",
        "IDirect3DDevice9_StretchRect",
        "IDirect3DDevice9_ColorFill",
        "IDirect3DDevice9_CreateOffscreenPlainSurface",
        "IDirect3DDevice9_SetRenderTarget",
        "IDirect3DDevice9_GetRenderTarget",
        "IDirect3DDevice9_SetDepthStencilSurface",
        "IDirect3DDevice9_GetDepthStencilSurface",
        "IDirect3DDevice9_BeginScene",
        "IDirect3DDevice9_EndScene",
        "IDirect3DDevice9_Clear",
        "IDirect3DDevice9_SetTransform",
        "IDirect3DDevice9_GetTransform",
        "IDirect3DDevice9_MultiplyTransform",
        "IDirect3DDevice9_SetViewport",
        "IDirect3DDevice9_GetViewport",
        "IDirect3DDevice9_SetMaterial",
        "IDirect3DDevice9_GetMaterial",
        "IDirect3DDevice9_SetLight",
        "IDirect3DDevice9_GetLight",
        "IDirect3DDevice9_LightEnable",
        "IDirect3DDevice9_GetLightEnable",
        "IDirect3DDevice9_SetClipPlane",
        "IDirect3DDevice9_GetClipPlane",
        "IDirect3DDevice9_SetRenderState",
        "IDirect3DDevice9_GetRenderState",
        "IDirect3DDevice9_CreateStateBlock",
        "IDirect3DDevice9_BeginStateBlock",
        "IDirect3DDevice9_EndStateBlock",
        "IDirect3DDevice9_SetClipStatus",
        "IDirect3DDevice9_GetClipStatus",
        "IDirect3DDevice9_GetTexture",
        "IDirect3DDevice9_SetTexture",
        "IDirect3DDevice9_GetTextureStageState",
        "IDirect3DDevice9_SetTextureStageState",
        "IDirect3DDevice9_GetSamplerState",
        "IDirect3DDevice9_SetSamplerState",
        "IDirect3DDevice9_ValidateDevice",
        "IDirect3DDevice9_SetPaletteEntries",
        "IDirect3DDevice9_GetPaletteEntries",
        "IDirect3DDevice9_SetCurrentTexturePalette",
        "IDirect3DDevice9_GetCurrentTexturePalette",
        "IDirect3DDevice9_SetScissorRect",
        "IDirect3DDevice9_GetScissorRect",
        "IDirect3DDevice9_SetSoftwareVertexProcessing",
        "IDirect3DDevice9_GetSoftwareVertexProcessing",
        "IDirect3DDevice9_SetNPatchMode",
        "IDirect3DDevice9_GetNPatchMode",
        "IDirect3DDevice9_DrawPrimitive",
        "IDirect3DDevice9_DrawIndexedPrimitive",
        "IDirect3DDevice9_DrawPrimitiveUP",
        "IDirect3DDevice9_DrawIndexedPrimitiveUP",
        "IDirect3DDevice9_ProcessVertices",
        "IDirect3DDevice9_CreateVertexDeclaration",
        "IDirect3DDevice9_SetVertexDeclaration",
        "IDirect3DDevice9_GetVertexDeclaration",
        "IDirect3DDevice9_SetFVF",
        "IDirect3DDevice9_GetFVF",
        "IDirect3DDevice9_CreateVertexShader",
        "IDirect3DDevice9_SetVertexShader",
        "IDirect3DDevice9_GetVertexShader",
        "IDirect3DDevice9_SetVertexShaderConstantF",
        "IDirect3DDevice9_GetVertexShaderConstantF",
        "IDirect3DDevice9_SetVertexShaderConstantI",
        "IDirect3DDevice9_GetVertexShaderConstantI",
        "IDirect3DDevice9_SetVertexShaderConstantB",
        "IDirect3DDevice9_GetVertexShaderConstantB",
        "IDirect3DDevice9_SetStreamSource",
        "IDirect3DDevice9_GetStreamSource",
        "IDirect3DDevice9_SetStreamSourceFreq",
        "IDirect3DDevice9_GetStreamSourceFreq",
        "IDirect3DDevice9_SetIndices",
        "IDirect3DDevice9_GetIndices",
        "IDirect3DDevice9_CreatePixelShader",
        "IDirect3DDevice9_SetPixelShader",
        "IDirect3DDevice9_GetPixelShader",
        "IDirect3DDevice9_SetPixelShaderConstantF",
        "IDirect3DDevice9_GetPixelShaderConstantF",
        "IDirect3DDevice9_SetPixelShaderConstantI",
        "IDirect3DDevice9_GetPixelShaderConstantI",
        "IDirect3DDevice9_SetPixelShaderConstantB",
        "IDirect3DDevice9_GetPixelShaderConstantB",
        "IDirect3DDevice9_DrawRectPatch",
        "IDirect3DDevice9_DrawTriPatch",
        "IDirect3DDevice9_DeletePatch",
        "IDirect3DDevice9_CreateQuery",
    ]
}

fn create_texture(
    context: &mut dyn HostCallContext,
    device: GuestAddress,
) -> Result<u32, Win32Error> {
    let width = context.argument_u32(1)?;
    let height = context.argument_u32(2)?;
    let _levels = context.argument_u32(3)?;
    let _usage = context.argument_u32(4)?;
    let format = context.argument_u32(5)?;
    let _pool = context.argument_u32(6)?;
    let out = GuestAddress(context.argument_u32(7)?);
    if width == 0 || height == 0 || out.0 == 0 {
        return Ok(E_INVALIDARG);
    }
    let pitch = width.saturating_mul(4);
    let bytes = pitch.saturating_mul(height);
    let bits = context.allocate_virtual_memory(bytes.max(4), true, true, false)?;
    let texture_format = match format {
        32 => TextureFormat::Rgba8Unorm,
        _ => TextureFormat::Bgra8Unorm,
    };
    let gpu = context
        .create_graphics_texture(TextureDescriptor {
            width,
            height,
            format: texture_format,
            render_target: false,
        })
        .ok();
    let methods = [
        "IUnknown_QueryInterface",
        "IUnknown_AddRef",
        "IUnknown_Release",
        "IDirect3DResource9_GetDevice",
        "IDirect3DResource9_SetPrivateData",
        "IDirect3DResource9_GetPrivateData",
        "IDirect3DResource9_FreePrivateData",
        "IDirect3DResource9_SetPriority",
        "IDirect3DResource9_GetPriority",
        "IDirect3DResource9_PreLoad",
        "IDirect3DResource9_GetType",
        "IDirect3DBaseTexture9_SetLOD",
        "IDirect3DBaseTexture9_GetLOD",
        "IDirect3DTexture9_GetLevelCount",
        "IDirect3DBaseTexture9_SetAutoGenFilterType",
        "IDirect3DBaseTexture9_GetAutoGenFilterType",
        "IDirect3DBaseTexture9_GenerateMipSubLevels",
        "IDirect3DTexture9_GetLevelDesc",
        "IDirect3DTexture9_GetSurfaceLevel",
        "IDirect3DTexture9_LockRect",
        "IDirect3DTexture9_UnlockRect",
        "IDirect3DTexture9_AddDirtyRect",
    ];
    let object = create_com_object(context, &methods, KIND_TEXTURE, 48)?;
    write_u32(context, GuestAddress(object.0 + 12), device.0)?;
    write_u32(context, GuestAddress(object.0 + 16), width)?;
    write_u32(context, GuestAddress(object.0 + 20), height)?;
    write_u32(context, GuestAddress(object.0 + 24), pitch)?;
    write_u32(context, GuestAddress(object.0 + 28), bits.0)?;
    write_u32(context, GuestAddress(object.0 + 32), bytes)?;
    if let Some(TextureId(id)) = gpu {
        write_u32(context, GuestAddress(object.0 + 36), id as u32)?;
        write_u32(context, GuestAddress(object.0 + 40), (id >> 32) as u32)?;
    }
    let _ = add_ref(context, device)?;
    write_u32(context, out, object.0)?;
    Ok(D3D_OK)
}

fn create_buffer(
    context: &mut dyn HostCallContext,
    device: GuestAddress,
    index: bool,
) -> Result<u32, Win32Error> {
    let length = context.argument_u32(1)?;
    let _usage = context.argument_u32(2)?;
    let _fmt_or_fvf = context.argument_u32(3)?;
    let _pool = context.argument_u32(4)?;
    let out = GuestAddress(context.argument_u32(5)?);
    if length == 0 || out.0 == 0 {
        return Ok(E_INVALIDARG);
    }
    let bits = context.allocate_virtual_memory(length, true, true, false)?;
    let methods: &[&str] = if index {
        &[
            "IUnknown_QueryInterface",
            "IUnknown_AddRef",
            "IUnknown_Release",
            "IDirect3DResource9_GetDevice",
            "IDirect3DResource9_SetPrivateData",
            "IDirect3DResource9_GetPrivateData",
            "IDirect3DResource9_FreePrivateData",
            "IDirect3DResource9_SetPriority",
            "IDirect3DResource9_GetPriority",
            "IDirect3DResource9_PreLoad",
            "IDirect3DResource9_GetType",
            "IDirect3DIndexBuffer9_Lock",
            "IDirect3DIndexBuffer9_Unlock",
            "IDirect3DIndexBuffer9_GetDesc",
        ]
    } else {
        &[
            "IUnknown_QueryInterface",
            "IUnknown_AddRef",
            "IUnknown_Release",
            "IDirect3DResource9_GetDevice",
            "IDirect3DResource9_SetPrivateData",
            "IDirect3DResource9_GetPrivateData",
            "IDirect3DResource9_FreePrivateData",
            "IDirect3DResource9_SetPriority",
            "IDirect3DResource9_GetPriority",
            "IDirect3DResource9_PreLoad",
            "IDirect3DResource9_GetType",
            "IDirect3DVertexBuffer9_Lock",
            "IDirect3DVertexBuffer9_Unlock",
            "IDirect3DVertexBuffer9_GetDesc",
        ]
    };
    let object = create_com_object(context, methods, if index { 7 } else { 6 }, 32)?;
    write_u32(context, GuestAddress(object.0 + 12), device.0)?;
    write_u32(context, GuestAddress(object.0 + 16), length)?;
    write_u32(context, GuestAddress(object.0 + 20), bits.0)?;
    let _ = add_ref(context, device)?;
    write_u32(context, out, object.0)?;
    Ok(D3D_OK)
}

fn lock_rect(context: &mut dyn HostCallContext, this: GuestAddress) -> Result<u32, Win32Error> {
    let _level = context.argument_u32(1)?;
    let locked = GuestAddress(context.argument_u32(2)?);
    let _rect = context.argument_u32(3)?;
    let _flags = context.argument_u32(4)?;
    let bits = read_u32(context, GuestAddress(this.0 + 28))?;
    let pitch = read_u32(context, GuestAddress(this.0 + 24))?;
    write_u32(context, locked, pitch)?;
    write_u32(context, GuestAddress(locked.0 + 4), bits)?;
    Ok(D3D_OK)
}

fn unlock_rect(context: &mut dyn HostCallContext, this: GuestAddress) -> Result<u32, Win32Error> {
    let bits = read_u32(context, GuestAddress(this.0 + 28))?;
    let bytes = read_u32(context, GuestAddress(this.0 + 32))? as usize;
    let gpu_lo = read_u32(context, GuestAddress(this.0 + 36))?;
    let gpu_hi = read_u32(context, GuestAddress(this.0 + 40))?;
    let id = u64::from(gpu_lo) | (u64::from(gpu_hi) << 32);
    if id != 0 && bytes != 0 {
        let mut pixels = vec![0_u8; bytes];
        context.read_memory(GuestAddress(bits), &mut pixels)?;
        let _ = context.write_graphics_texture(TextureId(id), &pixels);
    }
    Ok(D3D_OK)
}

fn lock_buffer(context: &mut dyn HostCallContext, this: GuestAddress) -> Result<u32, Win32Error> {
    let offset = context.argument_u32(1)?;
    let size = context.argument_u32(2)?;
    let out = GuestAddress(context.argument_u32(3)?);
    let _flags = context.argument_u32(4)?;
    let bits = read_u32(context, GuestAddress(this.0 + 20))?;
    let length = read_u32(context, GuestAddress(this.0 + 16))?;
    if offset > length || (size != 0 && offset.saturating_add(size) > length) {
        return Ok(E_INVALIDARG);
    }
    write_u32(context, out, bits.saturating_add(offset))?;
    Ok(D3D_OK)
}

fn get_surface_level(
    context: &mut dyn HostCallContext,
    this: GuestAddress,
) -> Result<u32, Win32Error> {
    let _level = context.argument_u32(1)?;
    let out = GuestAddress(context.argument_u32(2)?);
    let _ = add_ref(context, this)?;
    write_u32(context, out, this.0)?;
    Ok(D3D_OK)
}

fn get_level_desc(context: &mut dyn HostCallContext, this: GuestAddress) -> Result<u32, Win32Error> {
    let _level = context.argument_u32(1)?;
    let desc = GuestAddress(context.argument_u32(2)?);
    write_surface_desc(context, this, desc)
}

fn get_surface_desc(
    context: &mut dyn HostCallContext,
    this: GuestAddress,
) -> Result<u32, Win32Error> {
    let desc = GuestAddress(context.argument_u32(1)?);
    write_surface_desc(context, this, desc)
}

fn write_surface_desc(
    context: &mut dyn HostCallContext,
    this: GuestAddress,
    desc: GuestAddress,
) -> Result<u32, Win32Error> {
    let width = read_u32(context, GuestAddress(this.0 + 16)).unwrap_or(1);
    let height = read_u32(context, GuestAddress(this.0 + 20)).unwrap_or(1);
    let mut bytes = [0_u8; 32];
    bytes[0..4].copy_from_slice(&21_u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&1_u32.to_le_bytes());
    bytes[24..28].copy_from_slice(&width.to_le_bytes());
    bytes[28..32].copy_from_slice(&height.to_le_bytes());
    context.write_memory(desc, &bytes)?;
    Ok(D3D_OK)
}

fn get_back_buffer(
    context: &mut dyn HostCallContext,
    device: GuestAddress,
) -> Result<u32, Win32Error> {
    let _swap = context.argument_u32(1)?;
    let _index = context.argument_u32(2)?;
    let _type = context.argument_u32(3)?;
    let out = GuestAddress(context.argument_u32(4)?);
    let width = read_u32(context, GuestAddress(device.0 + 20)).unwrap_or(1280);
    let height = read_u32(context, GuestAddress(device.0 + 24)).unwrap_or(720);
    let pitch = width.saturating_mul(4);
    let bytes = pitch.saturating_mul(height);
    let bits = context.allocate_virtual_memory(bytes.max(4), true, true, false)?;
    let methods = [
        "IUnknown_QueryInterface",
        "IUnknown_AddRef",
        "IUnknown_Release",
        "IDirect3DResource9_GetDevice",
        "IDirect3DResource9_SetPrivateData",
        "IDirect3DResource9_GetPrivateData",
        "IDirect3DResource9_FreePrivateData",
        "IDirect3DResource9_SetPriority",
        "IDirect3DResource9_GetPriority",
        "IDirect3DResource9_PreLoad",
        "IDirect3DResource9_GetType",
        "IDirect3DSurface9_GetContainer",
        "IDirect3DSurface9_GetDesc",
        "IDirect3DSurface9_LockRect",
        "IDirect3DSurface9_UnlockRect",
    ];
    let surface = create_com_object(context, &methods, KIND_SURFACE, 48)?;
    write_u32(context, GuestAddress(surface.0 + 12), device.0)?;
    write_u32(context, GuestAddress(surface.0 + 16), width)?;
    write_u32(context, GuestAddress(surface.0 + 20), height)?;
    write_u32(context, GuestAddress(surface.0 + 24), pitch)?;
    write_u32(context, GuestAddress(surface.0 + 28), bits.0)?;
    write_u32(context, GuestAddress(surface.0 + 32), bytes)?;
    write_u32(context, out, surface.0)?;
    Ok(D3D_OK)
}

fn get_parent_d3d(context: &mut dyn HostCallContext, device: GuestAddress) -> Result<u32, Win32Error> {
    let out = GuestAddress(context.argument_u32(1)?);
    let d3d = read_u32(context, GuestAddress(device.0 + 12))?;
    if d3d != 0 {
        let _ = add_ref(context, GuestAddress(d3d))?;
    }
    write_u32(context, out, d3d)?;
    Ok(D3D_OK)
}

fn get_device_from_resource(
    context: &mut dyn HostCallContext,
    this: GuestAddress,
) -> Result<u32, Win32Error> {
    let out = GuestAddress(context.argument_u32(1)?);
    let device = read_u32(context, GuestAddress(this.0 + 12))?;
    if device != 0 {
        let _ = add_ref(context, GuestAddress(device))?;
    }
    write_u32(context, out, device)?;
    Ok(D3D_OK)
}

fn get_adapter_display_mode(context: &mut dyn HostCallContext) -> Result<u32, Win32Error> {
    let _adapter = context.argument_u32(1)?;
    let mode = GuestAddress(context.argument_u32(2)?);
    let (width, height) = context.primary_display_size();
    write_display_mode(context, mode, width, height)
}

fn get_display_mode_arg(context: &mut dyn HostCallContext) -> Result<u32, Win32Error> {
    let _swap = context.argument_u32(1)?;
    let mode = GuestAddress(context.argument_u32(2)?);
    let (width, height) = context.primary_display_size();
    write_display_mode(context, mode, width, height)
}

fn write_display_mode(
    context: &mut dyn HostCallContext,
    mode: GuestAddress,
    width: u32,
    height: u32,
) -> Result<u32, Win32Error> {
    let mut bytes = [0_u8; 16];
    bytes[0..4].copy_from_slice(&width.to_le_bytes());
    bytes[4..8].copy_from_slice(&height.to_le_bytes());
    bytes[8..12].copy_from_slice(&60_u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&22_u32.to_le_bytes());
    context.write_memory(mode, &bytes)?;
    Ok(D3D_OK)
}

fn get_device_caps(context: &mut dyn HostCallContext, from_d3d9: bool) -> Result<u32, Win32Error> {
    let caps = if from_d3d9 {
        GuestAddress(context.argument_u32(3)?)
    } else {
        GuestAddress(context.argument_u32(1)?)
    };
    let mut bytes = vec![0_u8; 304];
    bytes[0..4].copy_from_slice(&D3DDEVTYPE_HAL.to_le_bytes());
    bytes[4..8].copy_from_slice(&D3DADAPTER_DEFAULT.to_le_bytes());
    bytes[8..12].copy_from_slice(&0x0001_fffFu32.to_le_bytes());
    bytes[12..16].copy_from_slice(&0x0000_fffFu32.to_le_bytes());
    bytes[16..20].copy_from_slice(&0x0000_fffFu32.to_le_bytes());
    bytes[20..24].copy_from_slice(&0x8000_0001u32.to_le_bytes());
    bytes[24..28].copy_from_slice(&0x0001_fffFu32.to_le_bytes());
    bytes[88..92].copy_from_slice(&4096_u32.to_le_bytes());
    bytes[92..96].copy_from_slice(&4096_u32.to_le_bytes());
    context.write_memory(caps, &bytes)?;
    Ok(D3D_OK)
}

fn read_present_size(
    context: &dyn HostCallContext,
    params: GuestAddress,
) -> Result<(u32, u32), Win32Error> {
    if params.0 == 0 {
        return Ok(context.primary_display_size());
    }
    let mut header = [0_u8; 8];
    context.read_memory(params, &mut header)?;
    let width = u32::from_le_bytes(header[0..4].try_into().unwrap());
    let height = u32::from_le_bytes(header[4..8].try_into().unwrap());
    if width == 0 || height == 0 {
        Ok(context.primary_display_size())
    } else {
        Ok((width, height))
    }
}

fn create_com_object(
    context: &mut dyn HostCallContext,
    methods: &[&str],
    kind: u32,
    object_bytes: u32,
) -> Result<GuestAddress, Win32Error> {
    let module = context
        .loaded_module_handle(MODULE)
        .ok_or_else(|| Win32Error::ModuleNotFound(MODULE.to_owned()))?;
    let vtable_bytes = u32::try_from(methods.len().saturating_mul(4)).unwrap_or(u32::MAX);
    let vtable = context.allocate_virtual_memory(vtable_bytes.max(4), true, true, false)?;
    for (index, name) in methods.iter().enumerate() {
        let thunk = context.resolve_host_api(module, name)?;
        write_u32(
            context,
            GuestAddress(vtable.0 + u32::try_from(index * 4).unwrap_or(0)),
            thunk.0,
        )?;
    }
    let object = context.allocate_virtual_memory(object_bytes.max(12), true, true, false)?;
    write_u32(context, object, vtable.0)?;
    write_u32(context, GuestAddress(object.0 + 4), 1)?;
    write_u32(context, GuestAddress(object.0 + 8), kind)?;
    Ok(object)
}

fn query_interface(context: &mut dyn HostCallContext, this: GuestAddress) -> Result<u32, Win32Error> {
    let _iid = context.argument_u32(1)?;
    let out = GuestAddress(context.argument_u32(2)?);
    let _ = add_ref(context, this)?;
    write_u32(context, out, this.0)?;
    Ok(D3D_OK)
}

fn add_ref(context: &mut dyn HostCallContext, this: GuestAddress) -> Result<u32, Win32Error> {
    let count = read_u32(context, GuestAddress(this.0 + 4))?.saturating_add(1);
    write_u32(context, GuestAddress(this.0 + 4), count)?;
    Ok(count)
}

fn release(context: &mut dyn HostCallContext, this: GuestAddress) -> Result<u32, Win32Error> {
    let count = read_u32(context, GuestAddress(this.0 + 4))?.saturating_sub(1);
    write_u32(context, GuestAddress(this.0 + 4), count)?;
    Ok(count)
}

fn object_kind(context: &dyn HostCallContext, this: GuestAddress) -> Option<u32> {
    read_u32(context, GuestAddress(this.0 + 8)).ok()
}

fn read_u32(context: &dyn HostCallContext, address: GuestAddress) -> Result<u32, Win32Error> {
    let mut bytes = [0_u8; 4];
    context.read_memory(address, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn write_u32(
    context: &mut dyn HostCallContext,
    address: GuestAddress,
    value: u32,
) -> Result<(), Win32Error> {
    context.write_memory(address, &value.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_factory_and_com_methods() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        assert!(registry.len() > 50);
        assert!(
            registry
                .resolve(&ApiKey::new(MODULE, "Direct3DCreate9"))
                .is_some()
        );
        assert!(
            registry
                .resolve(&ApiKey::new(MODULE, "IDirect3D9_CreateDevice"))
                .is_some()
        );
        assert!(
            registry
                .resolve(&ApiKey::new(MODULE, "IDirect3DDevice9_Present"))
                .is_some()
        );
    }
}
