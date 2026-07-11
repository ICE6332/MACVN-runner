//! `wgpu` implementation of VNRT's backend-neutral graphics resources.

use std::collections::HashMap;

use thiserror::Error;
use vnrt_gfx::{GraphicsDevice, GraphicsError, TextureDescriptor, TextureFormat, TextureId};

/// Errors encountered while selecting a Host GPU adapter and device.
#[derive(Debug, Error)]
pub enum WgpuInitError {
    /// No adapter satisfies wgpu's baseline requirements.
    #[error("no compatible GPU adapter: {0}")]
    Adapter(#[from] wgpu::RequestAdapterError),
    /// The selected adapter could not create a logical device.
    #[error("failed to create GPU device: {0}")]
    Device(#[from] wgpu::RequestDeviceError),
}

struct TextureResource {
    descriptor: TextureDescriptor,
    texture: wgpu::Texture,
}

/// Real GPU resource owner backed by Metal, Vulkan, D3D12, or GLES.
pub struct WgpuGraphicsDevice {
    _instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_name: String,
    next_texture: u64,
    textures: HashMap<TextureId, TextureResource>,
}

impl WgpuGraphicsDevice {
    /// Select the platform's preferred high-performance adapter.
    pub fn new() -> Result<Self, WgpuInitError> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))?;
        let adapter_name = adapter.get_info().name;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("VNRT graphics device"),
                ..Default::default()
            }))?;
        Ok(Self {
            _instance: instance,
            device,
            queue,
            adapter_name,
            next_texture: 1,
            textures: HashMap::new(),
        })
    }
}

impl GraphicsDevice for WgpuGraphicsDevice {
    fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    fn create_texture(
        &mut self,
        descriptor: TextureDescriptor,
    ) -> Result<TextureId, GraphicsError> {
        let _ = descriptor.packed_len()?;
        let format = match descriptor.format {
            TextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            TextureFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
        };
        let mut usage = wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING;
        if descriptor.render_target {
            usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("VNRT Guest texture"),
            size: wgpu::Extent3d {
                width: descriptor.width,
                height: descriptor.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });
        let id = TextureId(self.next_texture);
        self.next_texture = self
            .next_texture
            .checked_add(1)
            .ok_or_else(|| GraphicsError::Backend("texture identifier exhausted".to_owned()))?;
        self.textures.insert(
            id,
            TextureResource {
                descriptor,
                texture,
            },
        );
        Ok(id)
    }

    fn write_texture(&mut self, texture: TextureId, bytes: &[u8]) -> Result<(), GraphicsError> {
        let resource = self
            .textures
            .get(&texture)
            .ok_or(GraphicsError::UnknownTexture(texture))?;
        let expected = resource.descriptor.packed_len()?;
        if bytes.len() != expected {
            return Err(GraphicsError::InvalidUploadLength {
                expected,
                actual: bytes.len(),
            });
        }
        self.queue.write_texture(
            resource.texture.as_image_copy(),
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(
                    resource.descriptor.width * resource.descriptor.format.bytes_per_pixel(),
                ),
                rows_per_image: Some(resource.descriptor.height),
            },
            wgpu::Extent3d {
                width: resource.descriptor.width,
                height: resource.descriptor.height,
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }

    fn destroy_texture(&mut self, texture: TextureId) -> bool {
        self.textures.remove(&texture).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a Host GPU adapter"]
    fn creates_and_uploads_a_guest_texture() {
        let mut graphics = WgpuGraphicsDevice::new().expect("Host GPU adapter");
        let descriptor = TextureDescriptor {
            width: 4,
            height: 4,
            format: TextureFormat::Rgba8Unorm,
            render_target: true,
        };
        let texture = graphics.create_texture(descriptor).expect("texture");
        graphics
            .write_texture(
                texture,
                &vec![0xff; descriptor.packed_len().expect("length")],
            )
            .expect("upload");
        assert!(graphics.destroy_texture(texture));
    }
}
