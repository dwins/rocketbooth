use image::RgbImage;
use rocketbooth_libav::{Frame, ScalingContext};

pub fn frame_to_image(frame: &Frame) -> Result<image::RgbImage, Box<dyn std::error::Error>> {
    let mut scaled_frame = None;
    if !frame.is_rgb24() {
        let mut dest = Frame::alloc_rgb24(frame.width() as i32, frame.height() as i32)
            .ok_or("Allocating temporary frame failed")?;
        let mut scaler = ScalingContext::new(
            frame.width() as i32,
            frame.height() as i32,
            frame.format(),
            frame.width() as i32,
            frame.height() as i32,
            dest.format(),
        );
        scaler.scale(frame, &mut dest);
        scaled_frame.insert(dest);
    }

    let frame = scaled_frame.as_ref().unwrap_or(frame);
    let bytes = Vec::from(frame.samples());
    RgbImage::from_vec(frame.width() as u32, frame.height() as u32, bytes)
        .ok_or_else(|| "Not enough bytes copied from frame".into())
}
