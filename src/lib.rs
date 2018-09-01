extern crate config;
extern crate libc;
extern crate raster;

use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

/// Parent module for ffmpeg/libav bindings.
pub mod av;

/// Parent module for linux event input (evdev) bindings.
pub mod evdev;

// / Parent module for OpenVG bindings (hardware accelerated vector graphics).
pub mod openvg;

/// Read-only application-wide settings from a config file.
pub struct PhotoboothConfig {
    cfg: config::types::Config,
}

impl PhotoboothConfig {
    /// Read the config file. Will panic if the file is invalid or doesn't exist
    pub fn load() -> PhotoboothConfig {
        let path = ::std::path::Path::new("rocketbooth.cfg");
        let cfg = ::config::reader::from_file(path).unwrap();
        PhotoboothConfig { cfg }
    }

    /// The source file path or url for the video input.
    /// This can be any file or virtual file that ffmpeg supports.
    /// If it is a real file, the format can be omitted, but generally for
    /// capture devices such as webcams it will be necessary to specify both.
    /// To use a USB webcam on Linux, this should be a video device such as
    /// /dev/video0 .
    pub fn source(&self) -> String {
        self.cfg.lookup_str("source").unwrap().to_string()
    }

    /// The source format for the video input.
    /// This can be any format or pseudo-format that ffmpeg supports.
    /// If the source is a real file, ffmpeg may be able to auto-detect, but
    /// generally for capture devices such as webcams it will be necessary to
    /// specify both.
    /// To use a USB webcam on Linux, this should be 'v4l2' so that the
    /// Video4Linux2 system is used to capture live video.
    pub fn format(&self) -> Option<av::format::Format> {
        self.cfg
            .lookup_str("format")
            .and_then(av::format::Format::lookup)
    }

    /// A list of key/value pairs specifying options for the video capture.
    /// These correspond directly to options that could be used at the ffmpeg
    /// command-line, and which options exactly are supported depends on the
    /// format used. All keys and values are expected to be strings, and will
    /// be interpreted by ffmpeg.  For example, if you tested an input
    /// resolution at the command line with 
    /// ```ffmpeg -video_size 640x480 -f v4l2 -i /dev/video0 out.mkv``
    /// then you could apply the same video size with 
    /// ```format_options = { video_size = "640x480"}```
    ///  in the config file.
    pub fn format_options(&self) -> av::util::Dictionary {
        use config::types::{ScalarValue, Value};
        let mut options = av::util::Dictionary::new();
        if let Option::Some(&Value::Group(ref settings)) = self.cfg.lookup("format_options") {
            settings.iter().for_each(|(k, v)| {
                if let Value::Svalue(ScalarValue::Str(ref prop)) = v.value {
                    options.set(k, prop)
                }
            });
        }
        options
    }

    /// A path prefix for image files from photobooth sessions.  This can be an
    /// absolute path, or relative to the working directory when the
    /// application is run.  The photos are named with a full timestamp of the
    /// time of capture in addition to the prefix.
    pub fn image_prefix(&self) -> String {
        self.cfg
            .lookup_str("image_prefix")
            .unwrap_or("")
            .to_string()
    }

    /// The device to watch for touch events.  This is the device name is
    /// reported by the device driver, NOT the /dev/input device node, so that
    /// configuration is not invalidated by initialization order variation,
    /// unplugging and replugging USB devices, etc.
    pub fn touch_device_name(&self) -> String {
        self.cfg
            .lookup_str("touch_device_name")
            .unwrap()
            .to_string()
    }

    /// A bool that controls whether we actually print the photobooth photos.
    /// Print jobs go to the system default print queue, so all other print
    /// settings should go through the CUPS configuration system.
    pub fn enable_printing(&self) -> bool {
        self.cfg.lookup_boolean_or("enable_printing", false)
    }

    /// A bool that controls whether we should shut down when the touchscreen
    /// is pressed for a long time (15 seconds). This allows a keyboard-less
    /// way to turn off the raspberry pi (since you might not have a keyboard
    /// connected to the kiosk.)
    pub fn enable_shutdown_on_longpress(&self) -> bool {
        self.cfg.lookup_boolean_or("enable_shutdown_on_longpress", false)
    }
}

/// Create an Image without zeroing out its buffer.
pub fn allocate_image(width: i32, height: i32) -> raster::Image {
    let buff_size = (width * height * 4) as usize;
    let mut bytes: Vec<u8> = Vec::with_capacity(buff_size);
    unsafe {
        bytes.set_len(buff_size);
    }
    raster::Image {
        width,
        height,
        bytes,
    }
}

/// Convert pixels from an av Frame into RGBA and write them to raster::Image.
pub fn copy_frame_to_image(
    scaler: &av::scale::ScalingContext,
    frame: &av::format::Frame,
    image: &mut ::raster::Image,
) {
    use av::util::AVPicture;
    let size = AVPicture::get_size(
        av::sys::AVPixelFormat_AV_PIX_FMT_RGBA,
        image.width,
        image.height,
    ) as usize;
    assert_eq!(image.bytes.len(), size);
    let pict = av::util::AVPicture::wrap(
        av::sys::AVPixelFormat_AV_PIX_FMT_RGBA,
        &mut image.bytes,
        image.width,
        image.height,
    );
    scaler.scale_picture(frame, &pict, 0, frame.borrow_avframe().height);
}

/// Find the scale and offsets needed to center an image in the display
/// Returns a tuple of (scale, dx, dy)
pub fn find_center(
    display_width: f32,
    display_height: f32,
    image_width: i32,
    image_height: i32,
) -> (f32, f32, f32) {
    let scale = {
        let h_scale = display_width / image_width as f32;
        let v_scale = display_height / image_height as f32;
        h_scale.min(v_scale)
    };

    let scaled_height = image_height as f32 * scale;
    let scaled_width = image_width as f32 * scale;

    let x_offset = (display_width - scaled_width) / 2.0;
    let y_offset = (display_height - scaled_height) / 2.0;
    (scale, x_offset, y_offset)
}

/// This is a macro that isn't handled well by bindgen, so we replicate the logic here.
fn eviocgname(size: usize) -> std::os::raw::c_ulong {
    use evdev::*;
    use std::os::raw::c_ulong;

    fn ioc(dir: c_ulong, type_: c_ulong, nr: c_ulong, size: c_ulong) -> c_ulong {
        (dir << _IOC_DIRSHIFT) | (type_ << _IOC_TYPESHIFT) | (nr << _IOC_NRSHIFT)
            | (size << _IOC_SIZESHIFT)
    }
    ioc(_IOC_READ, 'E' as c_ulong, 0x06, size as c_ulong)
}

/// Find an input device with the specified driver name.
pub fn find_device_with_name(name: &str) -> Option<std::fs::File> {
    use libc::ioctl;
    use std::fs::{read_dir, File};
    use std::os::unix::io::AsRawFd;

    let mut name_buffer = [0u8; 256];
    for potential_device in read_dir("/dev/input/").unwrap() {
        let potential_device = potential_device.unwrap();
        if potential_device
            .file_name()
            .to_string_lossy()
            .starts_with("event")
        {
            match File::open(potential_device.path()) {
                Result::Ok(file) => {
                    unsafe { ioctl(file.as_raw_fd(), eviocgname(256), name_buffer.as_mut_ptr()) };
                    let device_name = String::from_utf8_lossy(&name_buffer);
                    if &device_name[..name.len()] == name {
                        return Option::Some(file);
                    }
                }
                Result::Err(_) => {
                    // Couldn't read the file, probably not a useful device.
                    continue;
                }
            }
        }
    }
    Option::None
}

enum ColorDecodeMessage {
    // Contains a Frame to decode and write to the shared image.
    Decode(av::format::Frame),
    // No Frame is currently available for processing.
    Wait,
    // The video parser has been closed, so shut down the color decoding thread.
    Done,
}

pub struct VideoDecoder {
    active: Arc<Mutex<bool>>,
    worker: Option<JoinHandle<()>>,
}

impl VideoDecoder {
    pub fn new(
        config: Arc<PhotoboothConfig>,
        shared_image: Arc<Mutex<Option<raster::Image>>>,
    ) -> VideoDecoder {
        let active = Arc::new(Mutex::new(true));
        VideoDecoder {
            active: Arc::clone(&active),
            worker: Some(spawn_video_reading_thread(
                Arc::clone(&active),
                config,
                shared_image,
            )),
        }
    }
}

impl Drop for VideoDecoder {
    fn drop(&mut self) {
        *(self.active.lock().unwrap()) = false;
        let mut worker = None;
        std::mem::swap(&mut worker, &mut self.worker);
        if let Some(worker) = worker {
            worker.join().unwrap();
        }
    }
}

fn spawn_video_reading_thread(
    active: Arc<Mutex<bool>>,
    config: Arc<PhotoboothConfig>,
    shared_image: Arc<Mutex<Option<raster::Image>>>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("Video Read".to_string())
        .spawn(move || {
            let decoder_message = {
                let message = ColorDecodeMessage::Wait;
                let condvar = Condvar::new();
                Arc::new((Mutex::new(message), condvar))
            };
            let mut context = {
                let mut options = config.format_options();
                if let Option::Some(format) = config.format() {
                    av::format::FormatContext::open_format(&config.source(), &format, &mut options)
                } else {
                    av::format::FormatContext::open_input(&config.source(), &mut options)
                }.expect("Couldn't open video stream")
            };
            context.find_stream_info().expect("Read input metadata");
            let (mut video_decoder, video_idx) = {
                let stream = context
                    .get_streams()
                    .iter()
                    .find(|stream| {
                        stream.codec_ref().codec_type == av::sys::AVMediaType_AVMEDIA_TYPE_VIDEO
                    })
                    .expect("No video stream found");
                let decoder = av::codec::CodecContext::create_decode_context(stream.codec_ref())
                    .expect("Can't decode video");
                (decoder, stream.index)
            };
            video_decoder.open();
            let video_decoder = Arc::new((Mutex::new(video_decoder), Condvar::new()));

            let child_threads = {
                let mut threads = vec![
                    spawn_video_decoding_thread(
                        Arc::clone(&video_decoder),
                        Arc::clone(&decoder_message),
                    ),
                ];
                threads.extend((0..1).map(|i| {
                    spawn_color_conversion_thread(
                        i,
                        Arc::clone(&decoder_message),
                        Arc::clone(&shared_image),
                    )
                }));
                threads
            };

            let packet = av::util::Packet::new().unwrap();
            while *active.lock().unwrap() && context.read_into(&packet) {
                let stream_idx = packet.avpacket_ref().stream_index;
                if stream_idx == video_idx {
                    let handle = &video_decoder;
                    let mut codec = handle.0.lock().unwrap();
                    while !codec.send(&packet) {
                        codec = handle.1.wait(codec).unwrap();
                    }
                    handle.1.notify_all();
                }
                packet.release()
            }
            video_decoder.0.lock().unwrap().send_eof();
            video_decoder.1.notify_all();

            for handle in child_threads {
                handle.join().unwrap();
            }
        })
        .unwrap()
}

fn spawn_video_decoding_thread(
    video_decoder: Arc<(Mutex<av::codec::CodecContext>, Condvar)>,
    color_decode_message: Arc<(Mutex<ColorDecodeMessage>, Condvar)>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("Video decoder".to_string())
        .spawn(move || {
            let mut frame = av::format::Frame::new().unwrap();
            loop {
                use av::codec::ReceiveState;
                let decoder = video_decoder.0.lock().unwrap();
                let status = decoder.receive(&frame);
                match status {
                    ReceiveState::ERROR => break,
                    ReceiveState::DONE => break,
                    ReceiveState::PENDING => {
                        let _ignore = video_decoder.1.wait(decoder).unwrap();
                    }
                    ReceiveState::SUCCESS => {
                        video_decoder.1.notify_all();
                        *color_decode_message.0.lock().unwrap() = ColorDecodeMessage::Decode(frame);
                        color_decode_message.1.notify_one();
                        frame = av::format::Frame::new().unwrap();
                    }
                }
            }
            *color_decode_message.0.lock().unwrap() = ColorDecodeMessage::Done;
            color_decode_message.1.notify_all();
        })
        .unwrap()
}

fn spawn_color_conversion_thread(
    index: i32,
    message: Arc<(Mutex<ColorDecodeMessage>, Condvar)>,
    shared_image: Arc<Mutex<Option<raster::Image>>>,
) -> JoinHandle<()> {
    let name = format!("Recolor {}", index);
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(move || {
            let mut scaler = None;
            let mut image: Option<raster::Image> = None;

            loop {
                {
                    let mut frame = {
                        let mut lock = message.0.lock().unwrap();
                        while let ColorDecodeMessage::Wait = *lock {
                            lock = message.1.wait(lock).unwrap();
                        }
                        if let ColorDecodeMessage::Done = *lock {
                            break;
                        }
                        let mut local_message = ColorDecodeMessage::Wait;
                        ::std::mem::swap(&mut local_message, &mut lock);
                        match local_message {
                            ColorDecodeMessage::Decode(frame) => frame,
                            ColorDecodeMessage::Done | ColorDecodeMessage::Wait => {
                                panic!("Did not expect Done/Wait message after exiting Wait loop")
                            }
                        }
                    };
                    let (height, width, format) = {
                        let av = frame.borrow_avframe();
                        (av.height, av.width, av.format)
                    };

                    let scaler = scaler.get_or_insert_with(|| {
                        av::scale::ScalingContext::new(
                            width,
                            height,
                            unsafe { ::std::mem::transmute(format) },
                            width,
                            height,
                            av::sys::AVPixelFormat_AV_PIX_FMT_RGBA,
                            av::scale::SWSFlag::FAST_BILINEAR,
                        )
                    });

                    if image.is_some() {
                        let image = image.as_mut().unwrap();
                        if image.height != height || image.width != width {
                            *image = allocate_image(width, height);
                        }
                    } else {
                        image = Some(allocate_image(width, height));
                    }

                    copy_frame_to_image(&scaler, &frame, image.as_mut().unwrap());
                    frame.release();
                }
                let mut shared_image = shared_image.lock().unwrap();
                ::std::mem::swap(&mut image, &mut shared_image);
            }
        })
        .unwrap()
}
