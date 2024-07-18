use rocketbooth_libav::{Frame, ScalingContext};
use sdl2::{pixels::PixelFormatEnum, render::{Texture, TextureCreator}};

pub fn frame_to_texture<'t, T>(
    frame: &Frame,
    texture_creator: &'t TextureCreator<T>,
) -> Result<(FrameTextureUpdater, Texture<'t>), Box<dyn std::error::Error>> {
    let mut updater = FrameTextureUpdater { scaler: None };

    let format = if frame.is_rgb24() {
        Some(PixelFormatEnum::RGB24)
    } else if frame.is_yuyv422() {
        Some(PixelFormatEnum::YUY2)
    } else {
        let dest = Frame::alloc_rgb24(frame.width() as i32, frame.height() as i32)
            .ok_or("Allocating temporary frame failed")?;
        let scaler = ScalingContext::new(
            frame.width() as i32,
            frame.height() as i32,
            frame.format(),
            frame.width() as i32,
            frame.height() as i32,
            dest.format(),
        );
        updater.scaler = Some((scaler, dest));
        Some(PixelFormatEnum::RGB24)
    };

    let mut texture = texture_creator.create_texture(
        format,
        sdl2::render::TextureAccess::Static,
        frame.width() as u32,
        frame.height() as u32,
    )?;

    updater.update(frame, &mut texture)?;

    Ok((updater, texture))
}

pub struct FrameTextureUpdater {
    scaler: Option<(ScalingContext, Frame)>,
}

impl FrameTextureUpdater {
    pub fn update(
        &mut self,
        frame: &Frame,
        texture: &mut Texture,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let scaled_frame = self.scaler.as_mut().map(|(scaler, dest)| -> &Frame {
            scaler.scale(frame, dest);
            dest
        });

        let frame = scaled_frame.unwrap_or(frame);
        texture.update(None, frame.samples(), frame.pitch())?;
        Ok(())
    }
}
