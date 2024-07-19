use std::{
    sync::mpsc::{sync_channel, Receiver, SyncSender},
    thread::JoinHandle,
};

use rocketbooth_libav::{FormatContext, Frame, Packet, ReceiveResult, ScalingContext};
use sdl2::{
    pixels::PixelFormatEnum,
    render::{Texture, TextureCreator},
};

pub fn frame_to_texture<'t, T>(
    frame: &Frame,
    texture_creator: &'t TextureCreator<T>,
) -> Result<(FrameTextureUpdater, Texture<'t>), Box<dyn std::error::Error>> {
    let mut updater = FrameTextureUpdater {
        scaler: None,
        update_via: UpdateVia::RGB,
    };

    let format = if frame.is_rgb24() {
        Some(PixelFormatEnum::RGB24)
    } else if frame.is_yuv420p() {
        updater.update_via = UpdateVia::YUV;
        Some(PixelFormatEnum::IYUV)
    } else if frame.is_any_rgb_format() {
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
    } else {
        let dest = Frame::alloc_yuv420p(frame.width() as i32, frame.height() as i32)
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
        updater.update_via = UpdateVia::YUV;
        Some(PixelFormatEnum::IYUV)
    };

    let mut texture = texture_creator.create_texture(
        format,
        sdl2::render::TextureAccess::Streaming,
        frame.width() as u32,
        frame.height() as u32,
    )?;

    updater.update(frame, &mut texture)?;

    Ok((updater, texture))
}

enum UpdateVia {
    RGB,
    YUV,
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
            UpdateVia::RGB => texture.update(None, frame.samples(), frame.pitch())?,
            UpdateVia::YUV => {
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
    receiver: Receiver<Frame>,
    texture_creator: &'t TextureCreator<T>,
    updater_and_texture: Option<(FrameTextureUpdater, Texture<'t>)>,
}

impl<'t, T> FrameTextureManager<'t, T> {
    pub fn new(
        path: &str,
        texture_creator: &'t TextureCreator<T>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.to_owned();
        let updater_and_texture = None;
        let (sender, receiver) = sync_channel::<Frame>(0);
        let reader_thread_handle = std::thread::spawn(move || {
            Self::read_video_frames(&path, sender).ok();
        });
        Ok(Self {
            frame: None,
            receiver,
            texture_creator,
            updater_and_texture,
        })
    }

    pub fn frame_ref(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    pub fn texture_ref(&mut self) -> Option<&Texture<'t>> {
        if let Ok(frame) = self.receiver.try_recv() {
            match self.updater_and_texture.as_mut() {
                Some((updater, texture)) => updater.update(&frame, texture).ok()?,
                None => {
                    self.updater_and_texture = frame_to_texture(&frame, &self.texture_creator).ok()
                }
            }
            self.frame.insert(frame);
        }
        self.updater_and_texture
            .as_ref()
            .map(|(_, texture)| texture)
    }

    fn read_video_frames(
        src: &str,
        sender: SyncSender<Frame>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut context = FormatContext::open(src, None, None).ok_or("Failed to open file")?;
        context.find_stream_info();
        let video_stream = context
            .streams()
            .find(|stream| stream.is_video())
            .ok_or("No video stream")?;
        let mut decoder = video_stream
            .create_decoder()
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
                            sender.send(frame)?;
                            frame = Frame::new().ok_or("Failed to reinitialize frame")?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
