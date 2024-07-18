use std::path::Path;

use sdl2::render::{Texture, TextureCreator};


pub fn image_to_texture<T>(
    path: impl AsRef<Path>,
    texture_creator: &TextureCreator<T>,
) -> Result<Texture, Box<dyn std::error::Error>> {
    let img = image::io::Reader::open(path)?.decode()?.into_rgba8();
    let mut texture = texture_creator.create_texture(
        Some(sdl2::pixels::PixelFormatEnum::RGBA32),
        sdl2::render::TextureAccess::Static,
        img.width(),
        img.height(),
    )?;
    let flat_samples = img.as_flat_samples();
    texture.update(
        None,
        flat_samples.samples,
        flat_samples.layout.height_stride,
    )?;
    Ok(texture)
}
