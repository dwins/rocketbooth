extern crate rocketbooth;
extern crate raster;

use rocketbooth::av;
use rocketbooth::evdev;
use rocketbooth::openvg;
use std::env::args;
use std::io::Read;
use std::sync::{Arc,Mutex};
use std::thread::JoinHandle;

fn main() {
    av::format::register_all();

    let control = args().nth(1).unwrap();
    let control_min: i32 = args().nth(2).unwrap().parse().unwrap();
    let control_max: i32 = args().nth(3).unwrap().parse().unwrap();

    let config = Arc::new(rocketbooth::PhotoboothConfig::load());
    let image: Arc<Mutex<Option<raster::Image>>> = Arc::new(Mutex::new(None));
    spawn_render_thread(Arc::clone(&image));
    let _video_worker = rocketbooth::VideoDecoder::new(Arc::clone(&config), Arc::clone(&image));

    let mut device = rocketbooth::find_device_with_name(&config.touch_device_name()).expect("No matching input device.");
    let mut event_buff = vec![0u8; std::mem::size_of::<evdev::input_event>()];
    let x_scale = Rescale { 
        src_range: (0, 800), 
        dst_range: (control_min, control_max),
    };

    let (tx, rx) = std::sync::mpsc::channel();
    let processing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let thread_processing = std::sync::Arc::clone(&processing);
    std::thread::spawn(move || {
        loop {
            let value = rx.recv().unwrap();
            std::process::Command::new("v4l2-ctl")
                .arg(format!("--set-ctrl={}={}", control, value))
                .status()
                .expect("Failed to launch subprocess");
            thread_processing.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    });
    loop {
        device.read_exact(&mut event_buff).unwrap();
        if processing.load(std::sync::atomic::Ordering::Relaxed) {
            continue;
        }

        let event: evdev::input_event =
            unsafe { std::mem::transmute_copy(&* event_buff.as_ptr()) };
        if event.type_ == evdev::EV_ABS as u16 && (event.code == evdev::ABS_MT_POSITION_X as u16 || event.code == evdev::ABS_X as u16) {
            processing.store(true, std::sync::atomic::Ordering::Relaxed);
            tx.send(x_scale.scale(event.value)).unwrap();
        }
        if event.type_ == evdev::EV_ABS as u16 && (event.code == evdev::ABS_MT_POSITION_Y as u16 || event.code == evdev::ABS_Y as u16) {
            // println!("Y: {}", event.value);
        }
    }
}

struct Rescale {
    src_range: (i32, i32),
    dst_range: (i32, i32),
}

impl Rescale {
    fn scale(&self, x: i32) -> i32 {
        let &Rescale{ src_range, dst_range } = self;
        let src_span = (src_range.1 - src_range.0) as f32;
        let dst_span = (dst_range.1 - dst_range.0) as f32;
        let scaled = (x - src_range.0) as f32 * dst_span / src_span;
        scaled as i32 + dst_range.0
    }
}

fn spawn_render_thread(
    image: Arc<Mutex<Option<raster::Image>>>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("Render".to_string())
        .spawn(move || {
            let vg = openvg::OpenVG::init();
            let (display_width, display_height) = vg.get_display_size(0).unwrap();
            let src = openvg::VC_RECT_T {
                x: 0,
                y: 0,
                width: display_width << 16,
                height: display_height << 16,
            };
            let dest = openvg::VC_RECT_T {
                x: 0,
                y: 0,
                width: display_width,
                height: display_height,
            };
            let display = openvg::DispmanDisplay::open(0);
            let update = display.start_update();
            let element = update.element_add(1, &src, &dest);
            update.submit_sync();
            let mut window = element.to_window(display_width, display_height);
            let config = [
                openvg::sys::EGL_RED_SIZE,
                8,
                openvg::sys::EGL_GREEN_SIZE,
                8,
                openvg::sys::EGL_BLUE_SIZE,
                8,
                openvg::sys::EGL_ALPHA_SIZE,
                8,
                openvg::sys::EGL_LUMINANCE_SIZE,
                !0u32,
                openvg::sys::EGL_SURFACE_TYPE,
                openvg::sys::EGL_WINDOW_BIT | openvg::sys::EGL_PBUFFER_BIT,
                openvg::sys::EGL_SAMPLES,
                1,
            ];
            let egl_display = openvg::EGLDisplay::initialize(&mut window);
            let egl_config = egl_display.choose_config(&config);
            let egl_context = egl_config.create_context();
            let egl_surface = egl_config.create_window_surface(&mut window);
            egl_display.make_current(&egl_surface, &egl_surface, &egl_context);

            let mut frame_vg_image: Option<openvg::VGImage> = None;
            let mut offscreen_image: Option<openvg::VGImage> = None;
            let mut offscreen_surface: Option<openvg::EGLSurface> = None;

            loop {
                openvg::vg_setfv(
                    openvg::sys::VGParamType_VG_CLEAR_COLOR,
                    &[0.0, 0.0, 0.0, 1.0],
                );
                openvg::vg_seti(
                    openvg::sys::VGParamType_VG_BLEND_MODE,
                    openvg::sys::VGBlendMode_VG_BLEND_SRC_OVER as i32,
                );
                openvg::vg_seti(
                    openvg::sys::VGParamType_VG_MATRIX_MODE,
                    openvg::sys::VGMatrixMode_VG_MATRIX_IMAGE_USER_TO_SURFACE as i32,
                );

                openvg::vg_clear(0, 0, display_width, display_height);
                {
                    let image_option = image.lock().unwrap().take();

                    if let Some(image) = image_option {
                        let (fit_width, fit_height) = {
                            let display_aspect = display_width as f32 / display_height as f32;
                            let image_aspect = image.width as f32 / image.height as f32;
                            if display_aspect > image_aspect {
                                // display is wider than video. scaling based on vertical dimensions will ensure the full video frame fits.
                                (image.width * display_height / image.height, display_height)
                            } else {
                                (display_width, image.height * display_width / image.width)
                            }
                        };

                        let offscreen_image = offscreen_image.get_or_insert_with(|| {
                            openvg::VGImage::new(
                                openvg::sys::VGImageFormat_VG_sABGR_8888,
                                fit_width,
                                fit_height,
                                openvg::sys::VGImageQuality_VG_IMAGE_QUALITY_FASTER,
                            )
                        });

                        let offscreen_surface = offscreen_surface.get_or_insert_with(|| {
                            egl_config.create_image_surface(&offscreen_image)
                        });

                        let frame_vg_image = frame_vg_image.get_or_insert_with(|| {
                            openvg::VGImage::new(
                                openvg::sys::VGImageFormat_VG_sABGR_8888,
                                image.width,
                                image.height,
                                openvg::sys::VGImageQuality_VG_IMAGE_QUALITY_FASTER,
                            )
                        });

                        frame_vg_image.sub_data(
                            &image.bytes,
                            4 * image.width,
                            openvg::sys::VGImageFormat_VG_sABGR_8888,
                            0,
                            0,
                            image.width,
                            image.height,
                        );
                        egl_display.make_current(
                            &offscreen_surface,
                            &offscreen_surface,
                            &egl_context,
                        );
                        let (scale, dx, dy) = rocketbooth::find_center(
                            offscreen_image.width() as f32,
                            offscreen_image.height() as f32,
                            image.width,
                            image.height,
                        );
                        openvg::vg_load_identity();
                        openvg::vg_translate(dx, dy);
                        openvg::vg_scale(scale, scale);
                        openvg::vg_draw_image(&frame_vg_image);
                        egl_display.make_current(&egl_surface, &egl_surface, &egl_context);
                    }
                }

                openvg::vg_load_identity();

                if let Some(ref offscreen_image) = offscreen_image {
                    let (scale, x_offset, y_offset) = rocketbooth::find_center(
                        display_width as f32,
                        display_height as f32,
                        offscreen_image.width(),
                        offscreen_image.height(),
                    );
                    openvg::vg_translate(x_offset, display_height as f32 - y_offset);
                    openvg::vg_scale(scale, -scale);
                    openvg::vg_draw_image(&offscreen_image);
                }

                egl_surface.swap_buffers();
            }
        })
        .unwrap()
}