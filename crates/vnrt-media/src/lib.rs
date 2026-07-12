//! Bounded Host-side decoding for common visual-novel image and audio assets.
//!
//! Game-specific archives and encryption stay in the Guest compatibility
//! path. Once bytes have been extracted, this crate normalizes ordinary media
//! into RGBA8 pixels or interleaved `f32` PCM without involving a GPU backend.

use std::io::Cursor;

use image::{ImageFormat, ImageReader, Limits};
use symphonia::core::{
    codecs::audio::AudioDecoderOptions,
    errors::Error as SymphoniaError,
    formats::{FormatOptions, TrackType, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
};
use thiserror::Error;
use vnrt_gfx::{GraphicsDevice, GraphicsError, TextureDescriptor, TextureFormat, TextureId};

/// Maximum accepted width or height of one decoded image.
pub const MAX_IMAGE_DIMENSION: u32 = 16_384;
/// Maximum allocation owned by one decoded resource.
pub const MAX_DECODED_RESOURCE_BYTES: usize = 512 * 1024 * 1024;

/// One static image normalized for texture upload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Tightly packed, top-to-bottom RGBA8 pixels.
    pub rgba8: Vec<u8>,
}

/// Image encoding supplied by a resource table or filename when magic-byte
/// probing alone is ambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageResourceFormat {
    /// Portable Network Graphics.
    Png,
    /// JPEG/JFIF.
    Jpeg,
    /// Graphics Interchange Format.
    Gif,
    /// WebP.
    WebP,
    /// Windows bitmap.
    Bmp,
    /// Truevision TGA.
    Tga,
    /// Tagged Image File Format.
    Tiff,
    /// Windows icon.
    Ico,
    /// DirectDraw Surface texture.
    Dds,
}

impl ImageResourceFormat {
    const fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Gif => ImageFormat::Gif,
            Self::WebP => ImageFormat::WebP,
            Self::Bmp => ImageFormat::Bmp,
            Self::Tga => ImageFormat::Tga,
            Self::Tiff => ImageFormat::Tiff,
            Self::Ico => ImageFormat::Ico,
            Self::Dds => ImageFormat::Dds,
        }
    }
}

/// One audio stream normalized for a Host mixer.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudio {
    /// Samples per second.
    pub sample_rate: u32,
    /// Interleaved channel count.
    pub channels: u16,
    /// Interleaved `[-1.0, 1.0]` PCM frames.
    pub samples: Vec<f32>,
}

impl DecodedAudio {
    /// Number of complete sample frames.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.samples.len() / usize::from(self.channels)
        }
    }
}

/// Media probing or decoding failure.
#[derive(Debug, Error)]
pub enum MediaError {
    /// Input bytes do not identify a supported resource.
    #[error("unsupported or malformed media: {0}")]
    Invalid(String),
    /// The decoded resource exceeds the runtime's bounded allocation policy.
    #[error("decoded media exceeds limits: {0}")]
    Limit(&'static str),
    /// A decoded image could not be allocated or uploaded by the GPU backend.
    #[error(transparent)]
    Graphics(#[from] GraphicsError),
}

/// Decode PNG, JPEG, BMP, GIF, WebP, TGA, TIFF, ICO, or DDS bytes to RGBA8.
///
/// Animated formats currently return their first composited frame, matching
/// the static-texture contract used by the graphics layer.
pub fn decode_image(bytes: &[u8]) -> Result<DecodedImage, MediaError> {
    decode_image_inner(bytes, None)
}

/// Decode an image using a trusted container/extension format hint.
///
/// Use this for encodings such as TGA whose byte stream has no reliable magic
/// signature. The same output limits apply as with [`decode_image`].
pub fn decode_image_with_format(
    bytes: &[u8],
    format: ImageResourceFormat,
) -> Result<DecodedImage, MediaError> {
    decode_image_inner(bytes, Some(format.image_format()))
}

fn decode_image_inner(
    bytes: &[u8],
    format: Option<ImageFormat>,
) -> Result<DecodedImage, MediaError> {
    if bytes.is_empty() {
        return Err(MediaError::Invalid("empty image".to_owned()));
    }
    let mut reader = if let Some(format) = format {
        ImageReader::with_format(Cursor::new(bytes), format)
    } else {
        ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|error| MediaError::Invalid(error.to_string()))?
    };
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODED_RESOURCE_BYTES as u64);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| MediaError::Invalid(error.to_string()))?;
    let rgba = image.into_rgba8();
    let (width, height) = rgba.dimensions();
    let rgba8 = rgba.into_raw();
    if rgba8.len() > MAX_DECODED_RESOURCE_BYTES {
        return Err(MediaError::Limit("image pixel allocation"));
    }
    Ok(DecodedImage {
        width,
        height,
        rgba8,
    })
}

/// Decode one image and upload it to a backend-neutral RGBA8 texture.
///
/// If upload fails after allocation, the temporary texture is destroyed before
/// the error is returned.
pub fn upload_image_texture(
    graphics: &mut dyn GraphicsDevice,
    bytes: &[u8],
    render_target: bool,
) -> Result<TextureId, MediaError> {
    let image = decode_image(bytes)?;
    let texture = graphics.create_texture(TextureDescriptor {
        width: image.width,
        height: image.height,
        format: TextureFormat::Rgba8Unorm,
        render_target,
    })?;
    if let Err(error) = graphics.write_texture(texture, &image.rgba8) {
        graphics.destroy_texture(texture);
        return Err(error.into());
    }
    Ok(texture)
}

/// Decode WAV/PCM/ADPCM, OGG Vorbis, MP3, FLAC, AAC/M4A, or AIFF bytes.
pub fn decode_audio(bytes: &[u8]) -> Result<DecodedAudio, MediaError> {
    if bytes.is_empty() {
        return Err(MediaError::Invalid("empty audio".to_owned()));
    }
    let source = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let mut format = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            source,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(media_error)?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| MediaError::Invalid("audio stream has no decodable track".to_owned()))?;
    let track_id = track.id;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|parameters| parameters.audio())
        .cloned()
        .ok_or_else(|| MediaError::Invalid("audio codec parameters are missing".to_owned()))?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&codec_params, &AudioDecoderOptions::default())
        .map_err(media_error)?;
    let max_samples = MAX_DECODED_RESOURCE_BYTES / size_of::<f32>();
    let mut samples = Vec::new();
    let mut stream_spec = None;

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(error) => return Err(media_error(error)),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_) | SymphoniaError::IoError(_)) => continue,
            Err(error) => return Err(media_error(error)),
        };
        let spec = decoded.spec();
        let channels = u16::try_from(spec.channels().count())
            .map_err(|_| MediaError::Limit("audio channel count"))?;
        if channels == 0 || spec.rate() == 0 {
            return Err(MediaError::Invalid(
                "audio stream has an empty format".to_owned(),
            ));
        }
        match stream_spec {
            Some((rate, count)) if rate != spec.rate() || count != channels => {
                return Err(MediaError::Invalid(
                    "audio format changes inside one stream".to_owned(),
                ));
            }
            None => stream_spec = Some((spec.rate(), channels)),
            _ => {}
        }
        let mut converted = vec![0.0_f32; decoded.samples_interleaved()];
        decoded.copy_to_slice_interleaved(&mut converted);
        let new_len = samples
            .len()
            .checked_add(converted.len())
            .filter(|length| *length <= max_samples)
            .ok_or(MediaError::Limit("audio sample allocation"))?;
        samples.reserve(new_len - samples.len());
        samples.extend_from_slice(&converted);
    }

    let (sample_rate, channels) = stream_spec
        .ok_or_else(|| MediaError::Invalid("audio stream decoded no samples".to_owned()))?;
    Ok(DecodedAudio {
        sample_rate,
        channels,
        samples,
    })
}

fn media_error(error: SymphoniaError) -> MediaError {
    MediaError::Invalid(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{
        DynamicImage, ExtendedColorType, ImageEncoder, ImageFormat, codecs::png::PngEncoder,
    };
    use symphonia::core::codecs::audio::well_known::{
        CODEC_ID_AAC, CODEC_ID_ADPCM_IMA_WAV, CODEC_ID_FLAC, CODEC_ID_MP3, CODEC_ID_PCM_S16LE,
        CODEC_ID_VORBIS,
    };

    use super::*;

    #[derive(Default)]
    struct RecordingGraphics {
        descriptor: Option<TextureDescriptor>,
        pixels: Vec<u8>,
    }

    impl GraphicsDevice for RecordingGraphics {
        fn adapter_name(&self) -> &str {
            "test"
        }

        fn create_texture(
            &mut self,
            descriptor: TextureDescriptor,
        ) -> Result<TextureId, GraphicsError> {
            self.descriptor = Some(descriptor);
            Ok(TextureId(1))
        }

        fn write_texture(&mut self, texture: TextureId, bytes: &[u8]) -> Result<(), GraphicsError> {
            assert_eq!(texture, TextureId(1));
            self.pixels = bytes.to_vec();
            Ok(())
        }

        fn destroy_texture(&mut self, texture: TextureId) -> bool {
            texture == TextureId(1)
        }
    }

    #[test]
    fn decodes_png_to_rgba8() {
        let mut encoded = Vec::new();
        PngEncoder::new(&mut encoded)
            .write_image(&[0x12, 0x34, 0x56, 0xff], 1, 1, ExtendedColorType::Rgba8)
            .unwrap();

        let decoded = decode_image(&encoded).unwrap();
        assert_eq!(decoded.width, 1);
        assert_eq!(decoded.height, 1);
        assert_eq!(decoded.rgba8, [0x12, 0x34, 0x56, 0xff]);
    }

    #[test]
    fn decoded_image_uploads_through_shared_graphics_contract() {
        let mut encoded = Vec::new();
        PngEncoder::new(&mut encoded)
            .write_image(&[1, 2, 3, 4], 1, 1, ExtendedColorType::Rgba8)
            .unwrap();
        let mut graphics = RecordingGraphics::default();

        assert_eq!(
            upload_image_texture(&mut graphics, &encoded, false).unwrap(),
            TextureId(1)
        );
        assert_eq!(
            graphics.descriptor,
            Some(TextureDescriptor {
                width: 1,
                height: 1,
                format: TextureFormat::Rgba8Unorm,
                render_target: false,
            })
        );
        assert_eq!(graphics.pixels, [1, 2, 3, 4]);
    }

    #[test]
    fn enabled_common_image_formats_round_trip() {
        for format in [
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Gif,
            ImageFormat::WebP,
            ImageFormat::Bmp,
            ImageFormat::Tga,
            ImageFormat::Tiff,
            ImageFormat::Ico,
        ] {
            let image = if format == ImageFormat::Jpeg {
                DynamicImage::new_rgb8(2, 1)
            } else {
                DynamicImage::new_rgba8(2, 1)
            };
            let mut encoded = Cursor::new(Vec::new());
            image.write_to(&mut encoded, format).unwrap();
            let hinted = match format {
                ImageFormat::Png => ImageResourceFormat::Png,
                ImageFormat::Jpeg => ImageResourceFormat::Jpeg,
                ImageFormat::Gif => ImageResourceFormat::Gif,
                ImageFormat::WebP => ImageResourceFormat::WebP,
                ImageFormat::Bmp => ImageResourceFormat::Bmp,
                ImageFormat::Tga => ImageResourceFormat::Tga,
                ImageFormat::Tiff => ImageResourceFormat::Tiff,
                ImageFormat::Ico => ImageResourceFormat::Ico,
                _ => unreachable!(),
            };
            let decoded = decode_image_with_format(encoded.get_ref(), hinted).unwrap();
            assert_eq!((decoded.width, decoded.height), (2, 1), "{format:?}");
        }
    }

    #[test]
    fn decodes_pcm_wave_to_interleaved_f32() {
        let wave = pcm16_wave(&[i16::MIN, 0, i16::MAX], 22_050, 1);
        let decoded = decode_audio(&wave).unwrap();
        assert_eq!(decoded.sample_rate, 22_050);
        assert_eq!(decoded.channels, 1);
        assert_eq!(decoded.frame_count(), 3);
        assert!(decoded.samples[0] <= -0.999);
        assert_eq!(decoded.samples[1], 0.0);
        assert!(decoded.samples[2] >= 0.999);
    }

    #[test]
    fn common_audio_decoders_are_registered() {
        let codecs = symphonia::default::get_codecs();
        for codec in [
            CODEC_ID_PCM_S16LE,
            CODEC_ID_ADPCM_IMA_WAV,
            CODEC_ID_VORBIS,
            CODEC_ID_MP3,
            CODEC_ID_FLAC,
            CODEC_ID_AAC,
        ] {
            assert!(codecs.get_audio_decoder(codec).is_some(), "{codec:?}");
        }
    }

    fn pcm16_wave(samples: &[i16], sample_rate: u32, channels: u16) -> Vec<u8> {
        let data_size = u32::try_from(samples.len() * 2).unwrap();
        let mut bytes = Vec::with_capacity(44 + data_size as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }
}
