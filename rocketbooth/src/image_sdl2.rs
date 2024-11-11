use std::path::Path;

use sdl2::render::{Texture, TextureCreator};

pub fn image_to_texture<T>(
    path: impl AsRef<Path>,
    texture_creator: &TextureCreator<T>,
) -> Result<Texture, Box<dyn std::error::Error>> {
    let rgb_image = image::io::Reader::open(path)?.decode()?.into_rgba8();
    loaded_image_to_texture(rgb_image, texture_creator)
}

pub fn loaded_image_to_texture<T>(
    image: image::RgbaImage,
    texture_creator: &TextureCreator<T>,
) -> crate::Result<Texture> {
    let mut texture = texture_creator.create_texture(
        Some(sdl2::pixels::PixelFormatEnum::RGBA32),
        sdl2::render::TextureAccess::Static,
        image.width(),
        image.height(),
    )?;

    texture.set_blend_mode(sdl2::render::BlendMode::Blend);
    let flat_samples = image.as_flat_samples();
    texture.update(
        None,
        flat_samples.samples,
        flat_samples.layout.height_stride,
    )?;
    Ok(texture)
}