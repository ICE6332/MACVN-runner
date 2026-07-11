//! Backend-neutral graphics resources shared by Guest graphics APIs.

use thiserror::Error;

/// Stable runtime identifier for one graphics texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureId(pub u64);

/// Pixel formats needed by the initial Direct3D and GDI presentation paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    /// Eight-bit red, green, blue, and alpha channels.
    Rgba8Unorm,
    /// Eight-bit blue, green, red, and alpha channels.
    Bgra8Unorm,
}

impl TextureFormat {
    /// Bytes occupied by one pixel.
    #[must_use]
    pub const fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::Rgba8Unorm | Self::Bgra8Unorm => 4,
        }
    }
}

/// Backend-neutral description of a two-dimensional Guest texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureDescriptor {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Pixel encoding.
    pub format: TextureFormat,
    /// Whether the texture may be selected as a render target.
    pub render_target: bool,
}

impl TextureDescriptor {
    /// Validate dimensions and return the tightly packed byte length.
    pub fn packed_len(self) -> Result<usize, GraphicsError> {
        if self.width == 0 || self.height == 0 {
            return Err(GraphicsError::InvalidTextureSize {
                width: self.width,
                height: self.height,
            });
        }
        let length = u64::from(self.width)
            .checked_mul(u64::from(self.height))
            .and_then(|pixels| pixels.checked_mul(u64::from(self.format.bytes_per_pixel())))
            .and_then(|bytes| usize::try_from(bytes).ok())
            .ok_or(GraphicsError::TextureSizeOverflow)?;
        Ok(length)
    }
}

/// Errors exposed by the backend-neutral graphics layer.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GraphicsError {
    /// A zero-sized texture cannot be represented by the selected GPU backend.
    #[error("invalid texture size {width}x{height}")]
    InvalidTextureSize {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
    },
    /// Texture dimensions overflow Host addressable memory.
    #[error("texture byte length overflow")]
    TextureSizeOverflow,
    /// A resource identifier does not refer to a live texture.
    #[error("unknown texture {0:?}")]
    UnknownTexture(TextureId),
    /// Upload bytes do not match the destination texture layout.
    #[error("texture upload length {actual} does not match expected length {expected}")]
    InvalidUploadLength {
        /// Required byte length.
        expected: usize,
        /// Supplied byte length.
        actual: usize,
    },
    /// The selected Host backend could not complete an operation.
    #[error("graphics backend failed: {0}")]
    Backend(String),
}

/// Minimal resource contract implemented by the selected Host GPU backend.
pub trait GraphicsDevice: Send {
    /// Human-readable adapter/backend name for diagnostics.
    fn adapter_name(&self) -> &str;
    /// Allocate a texture and return its stable runtime identifier.
    fn create_texture(&mut self, descriptor: TextureDescriptor)
    -> Result<TextureId, GraphicsError>;
    /// Replace a complete texture with tightly packed pixel data.
    fn write_texture(&mut self, texture: TextureId, bytes: &[u8]) -> Result<(), GraphicsError>;
    /// Destroy a texture, returning whether it was live.
    fn destroy_texture(&mut self, texture: TextureId) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_packed_texture_length() {
        let descriptor = TextureDescriptor {
            width: 800,
            height: 600,
            format: TextureFormat::Bgra8Unorm,
            render_target: true,
        };
        assert_eq!(descriptor.packed_len(), Ok(1_920_000));
    }
}
