# Common media resources

`vnrt-media` is the Host-side normalization boundary for ordinary resources
after a Guest engine has extracted any game-specific archive or encryption.

Image input:

- PNG, JPEG, BMP, GIF, WebP, TGA, TIFF, ICO, and DDS.
- Output is bounded, tightly packed RGBA8.
- Animated inputs currently use the first composited frame.
- Self-describing formats can be probed from bytes; TGA/DDS callers can pass an
  explicit resource-format hint when the container or filename already knows it.
- Decoded images can upload directly through the shared `vnrt-gfx` contract.

Audio input:

- WAV with PCM or ADPCM, OGG Vorbis, MP3, FLAC, AAC/M4A, and AIFF.
- Output is interleaved `f32` PCM with explicit sample rate and channel count.

Decoding uses the pure-Rust `image` and Symphonia libraries. It is independent
of Metal/wgpu and does not replace the selected game's own YPF reader or Guest
decoder path.
