use std::sync::{Arc, Mutex};

use rocketbooth_libav::{
    Dictionary, Format, FormatContext, Frame, Packet, ReceiveResult, ScalingContext,
};
use sdl2::{
    pixels::PixelFormatEnum,
    render::{Texture, TextureCreator},
};

use crate::VideoSource;

pub fn frame_to_texture<'t, T>(
    frame: &Frame,
    display_size: Option<(usize, usize)>,
    texture_creator: &'t TextureCreator<T>,
) -> Result<(FrameTextureUpdater, Texture<'t>), Box<dyn std::error::Error>> {
    let mut updater = FrameTextureUpdater {
        scaler: None,
        update_via: UpdateVia::Rgb,
    };

    let size_is_compatible =
        display_size.map_or(true, |size| (frame.width(), frame.height()) == size);
    let (dest_width, dest_height) = display_size.map_or_else(
        || (frame.width() as i32, frame.height() as i32),
        |(w, h)| (w as i32, h as i32),
    );

    let format = if frame.is_rgb24() && size_is_compatible {
        Some(PixelFormatEnum::RGB24)
    } else if frame.is_yuv420p() && size_is_compatible {
        updater.update_via = UpdateVia::Yuv;
        Some(PixelFormatEnum::IYUV)
    } else if frame.is_any_rgb_format() {
        let dest = Frame::alloc_rgb24(dest_width, dest_height)
            .ok_or("Allocating temporary frame failed")?;
        let scaler = ScalingContext::new(
            frame.width() as i32,
            frame.height() as i32,
            frame.format(),
            dest_width,
            dest_height,
            dest.format(),
        );
        updater.scaler = Some((scaler, dest));
        Some(PixelFormatEnum::RGB24)
    } else {
        let dest = Frame::alloc_yuv420p(dest_width, dest_height)
            .ok_or("Allocating temporary frame failed")?;
        let scaler = ScalingContext::new(
            frame.width() as i32,
            frame.height() as i32,
            frame.format(),
            dest_width,
            dest_height,
            dest.format(),
        );
        updater.scaler = Some((scaler, dest));
        updater.update_via = UpdateVia::Yuv;
        Some(PixelFormatEnum::IYUV)
    };

    let mut texture = texture_creator.create_texture(
        format,
        sdl2::render::TextureAccess::Streaming,
        dest_width as _,
        dest_height as _,
    )?;

    updater.update(frame, &mut texture)?;

    Ok((updater, texture))
}

enum UpdateVia {
    Rgb,
    Yuv,
}

pub struct FrameTextureUpdater {
    scaler: Option<(ScalingContext, Frame)>,
    update_via: UpdateVia,
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
        match self.update_via {
            UpdateVia::Rgb => texture.update(None, frame.samples(), frame.pitch())?,
            UpdateVia::Yuv => {
                let yuv_samples = frame.yuv_samples();
                texture.update_yuv(
                    None,
                    yuv_samples.y_samples,
                    yuv_samples.y_pitch,
                    yuv_samples.u_samples,
                    yuv_samples.u_pitch,
                    yuv_samples.v_samples,
                    yuv_samples.v_pitch,
                )?;
            }
        }
        Ok(())
    }
}

pub struct FrameTextureManager<'t, T> {
    frame: Option<Frame>,
    shared_frame: Arc<Mutex<Option<Frame>>>,
    texture_creator: &'t TextureCreator<T>,
    updater_and_texture: Option<(FrameTextureUpdater, Texture<'t>)>,
    display_size: Option<(usize, usize)>,
}

impl<'t, T> FrameTextureManager<'t, T> {
    pub fn new(
        video_source: &VideoSource,
        texture_creator: &'t TextureCreator<T>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let path = video_source.path.clone();
        let format = video_source.format.as_deref().and_then(Format::from_name);
        let video_codec = video_source.video_codec.clone();
        let display_size = video_source.display_size;
        let options = if video_source.options.is_empty() {
            None
        } else {
            Some(Dictionary::from(&video_source.options))
        };
        let updater_and_texture = None;
        let shared_frame = Arc::new(Mutex::new(None));
        std::thread::spawn({
            let shared_frame = Arc::clone(&shared_frame);
            move || {
                if let Err(e) = Self::read_video_frames(
                    path.as_str(),
                    format,
                    video_codec,
                    options,
                    shared_frame,
                ) {
                    println!("{e:?}");
                }
            }
        });
        Ok(Self {
            frame: None,
            shared_frame,
            texture_creator,
            updater_and_texture,
            display_size,
        })
    }

    pub fn frame_ref(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    pub fn texture_ref(&mut self) -> Option<&Texture<'t>> {
        if let Some(frame) = self
            .shared_frame
            .lock()
            .as_mut()
            .ok()
            .and_then(|frame| frame.take())
        {
            match self.updater_and_texture.as_mut() {
                Some((updater, texture)) => updater.update(&frame, texture).ok()?,
                None => {
                    self.updater_and_texture =
                        frame_to_texture(&frame, self.display_size, self.texture_creator).ok()
                }
            }
            self.frame = Some(frame);
        }
        self.updater_and_texture
            .as_ref()
            .map(|(_, texture)| texture)
    }

    fn read_video_frames(
        src: &str,
        format: Option<Format>,
        video_codec: Option<String>,
        options: Option<Dictionary>,
        shared_frame: Arc<Mutex<Option<Frame>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut context = FormatContext::open(src, format, options).ok_or("Failed to open file")?;
        context.find_stream_info();
        let video_stream = context
            .streams()
            .find(|stream| stream.is_video())
            .ok_or("No video stream")?;
        let mut decoder = video_stream
            .create_decoder(video_codec.as_deref())
            .ok_or("Codec failed to initialize")?;
        let mut packet = Packet::new().ok_or("Could not allocate packet")?;
        'read: while context.read_into(&mut packet) {
            if packet.stream_index() == video_stream.index() {
                decoder.send(&mut packet);
                let mut frame = Frame::new().ok_or("Failed to initialize frame")?;
                'receive: loop {
                    let result = decoder.receive(&mut frame);
                    match result {
                        ReceiveResult::Done => break 'read,
                        ReceiveResult::Pending | ReceiveResult::Error => break 'receive,
                        ReceiveResult::Success => {
                            *shared_frame.lock().unwrap() = Some(frame);
                            frame = Frame::new().ok_or("Failed to reinitialize frame")?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
