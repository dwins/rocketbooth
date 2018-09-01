extern crate config;
extern crate cups_sys;
extern crate libc;
extern crate rocketbooth;
extern crate raster;
extern crate time;

use std::io::Read;
use std::sync::{Arc, Condvar, Mutex};
use std::sync::mpsc;
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, Instant};
use rocketbooth::av::format::register_all;
use rocketbooth::evdev::*;
use rocketbooth::openvg;
use raster::{save, BlendMode, Image, PositionMode};
use raster::editor::blend;

/// The state of the app's workflow that we are in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppState {
    Ready,
    Welcome,
    Preview { index: usize },
    WaitForPrint,
}

fn main() {
    register_all(); // Load ffmpeg file decoders.

    let config = Arc::new(rocketbooth::PhotoboothConfig::load());
    let frame_image: Arc<Mutex<Option<raster::Image>>> = Arc::new(Mutex::new(None));
    let captures = Arc::new(Mutex::<Option<Vec<Image>>>::new(None));
    let next_change_time = Arc::new(Mutex::new(Instant::now()));
    let appstate = Arc::new((Mutex::new(AppState::Ready), Condvar::new()));
    let (print_sender, print_receiver) = mpsc::channel::<()>();
    let print_barrier = Arc::new(std::sync::Barrier::new(2));
    let button_state = Arc::new((Mutex::new(false), Condvar::new()));

    spawn_render_thread(
        Arc::clone(&frame_image),
        Arc::clone(&appstate),
        Arc::clone(&captures),
        Arc::clone(&next_change_time),
    );

    spawn_appstate_update_thread(
        Arc::clone(&appstate),
        Arc::clone(&frame_image),
        Arc::clone(&config),
        Arc::clone(&next_change_time),
        print_sender,
        Arc::clone(&print_barrier),
    );

    spawn_printer_thread(
        Arc::clone(&config),
        Arc::clone(&captures),
        print_receiver,
        Arc::clone(&print_barrier),
    );

    if config.enable_shutdown_on_longpress() {
        spawn_shutdown_thread(
            Arc::clone(&button_state),
        );
    }

    // Loop and notify the appstate condition variable if the screen is touched.
    // TODO: Exit if the touch lasts for 5 seconds
    let mut device = rocketbooth::find_device_with_name(&config.touch_device_name())
        .expect("No matching input device");
    let mut event_buff = vec![0u8; std::mem::size_of::<input_event>()];
    let mut prev = false;
    loop {
        device.read_exact(&mut event_buff).unwrap();
        let event: input_event = unsafe { std::mem::transmute_copy(&*event_buff.as_ptr()) };
        if event.type_ == EV_KEY as u16 && event.code == BTN_TOUCH as u16 {
            let pressed = event.value != 0;
            if pressed != prev {
                prev = pressed;
                if prev {
                    *button_state.0.lock().unwrap() = pressed;
                    button_state.1.notify_all();
                    appstate.1.notify_all()
                }
            }
        }
    }
}

fn spawn_shutdown_thread(
    button_state: Arc<(Mutex<bool>, Condvar)>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("Shutdown".to_string())
        .spawn(move || {
            let (ref state, ref cvar) = *button_state;
            let mut pressed = state.lock().unwrap();
            loop {
                if *pressed {
                    let (pressed_, timeout_result) = cvar.wait_timeout(pressed, Duration::from_secs(15)).unwrap();
                    pressed = pressed_;
                    if timeout_result.timed_out() {
                        std::process::Command::new("sudo")
                            .arg("shutdown")
                            .arg("now")
                            .status()
                            .expect("Failed to launch subprocess");
                        std::process::exit(0);
                    }
                }
                pressed = cvar.wait(pressed).unwrap();
            }
        }).unwrap()
}

fn spawn_render_thread(
    frame_image: Arc<Mutex<Option<raster::Image>>>,
    appstate: Arc<(Mutex<AppState>, Condvar)>,
    captures: Arc<Mutex<Option<Vec<Image>>>>,
    next_change_time: Arc<Mutex<Instant>>,
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

            let mut offscreen_image: Option<openvg::VGImage> = None;
            let mut offscreen_surface: Option<openvg::EGLSurface> = None;

            let mut vg_images: Option<Vec<openvg::VGImage>> = None;
            let mut frame_vg_image: Option<openvg::VGImage> = None;
            let (title_card, title_image) = load_raster_and_image("prompts/prompts.001.png");
            let (instruction_card, instruction_image) =
                load_raster_and_image("prompts/prompts.002.png");
            let counters = (3..7)
                .rev()
                .map(|i| {
                    let fname = format!("prompts/prompts.{:03}.png", i);
                    load_raster_and_image(&fname)
                })
                .collect::<Vec<_>>();
            let (print_card, print_image) = load_raster_and_image("prompts/prompts.007.png");
            let (_, shadow_image) = load_raster_and_image("shadow.png");
            let mut four_up_context = FourUpContext::new();

            loop {
                let appstate: AppState = { *appstate.0.lock().unwrap() };
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
                    let image_option = frame_image.lock().unwrap().take();

                    if let Some(mut image) = image_option {
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
                        captures.lock().unwrap().get_or_insert_with(|| {
                            vec![rocketbooth::allocate_image(image.width, image.height); 4]
                        });
                        let vg_images = vg_images.get_or_insert_with(|| {
                            (0..4)
                                .map(|_| {
                                    openvg::VGImage::new(
                                        openvg::sys::VGImageFormat_VG_sABGR_8888,
                                        fit_width,
                                        fit_height,
                                        openvg::sys::VGImageQuality_VG_IMAGE_QUALITY_FASTER,
                                    )
                                })
                                .collect()
                        });

                        let preview_index = match appstate {
                            AppState::Welcome => Some(0),
                            AppState::Preview { index } => Some(index),
                            AppState::Ready | AppState::WaitForPrint => None,
                        };

                        if let Some(i) = preview_index {
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
                            offscreen_image.copy_image(
                                &mut vg_images[i],
                                0,
                                0,
                                0,
                                0,
                                offscreen_image.width(),
                                offscreen_image.height(),
                                false,
                            );
                            {
                                let mut captures = captures.lock().unwrap();
                                let captures = captures.as_mut().unwrap();
                                std::mem::swap(&mut image, &mut captures[i]);
                            }
                        }
                    }
                }

                openvg::vg_load_identity();
                match appstate {
                    AppState::Ready => {
                        let (scale, x_offset, y_offset) = rocketbooth::find_center(
                            display_width as f32,
                            display_height as f32,
                            title_card.width,
                            title_card.height,
                        );
                        openvg::vg_translate(x_offset, display_height as f32 - y_offset);
                        openvg::vg_scale(scale, -scale);
                        openvg::vg_draw_image(&title_image);
                    }
                    AppState::Welcome => {
                        if let Some(vg_images) = vg_images.as_ref() {
                            let (scale, x_offset, y_offset) = rocketbooth::find_center(
                                display_width as f32,
                                display_height as f32,
                                vg_images[0].width(),
                                vg_images[0].height(),
                            );
                            openvg::vg_translate(x_offset, display_height as f32 - y_offset);
                            openvg::vg_scale(scale, -scale);
                            four_up_context.display(&vg_images[0], &DisplayStyle::Mirrored);
                            openvg::vg_load_identity();
                        }
                        let (scale, x_offset, y_offset) = rocketbooth::find_center(
                            display_width as f32,
                            display_height as f32,
                            instruction_card.width,
                            instruction_card.height,
                        );
                        openvg::vg_translate(x_offset, display_height as f32 - y_offset);
                        openvg::vg_scale(scale, -scale);
                        openvg::vg_draw_image(&instruction_image);
                    }
                    AppState::Preview { index } => {
                        if let Some(vg_images) = vg_images.as_ref() {
                            let next_change_time = next_change_time.lock().unwrap();
                            let now = Instant::now();
                            let time_remaining = if now > *next_change_time {
                                0usize
                            } else {
                                next_change_time.duration_since(now).as_secs().min(3).max(0)
                                    as usize
                            };
                            let mut styles = vec![DisplayStyle::Overlay(&shadow_image); index];
                            styles.push(DisplayStyle::MirroredOverlay(&counters[time_remaining].1));
                            let (scale, x_offset, y_offset) = rocketbooth::find_center(
                                display_width as f32,
                                display_height as f32,
                                vg_images[0].width(),
                                vg_images[0].height(),
                            );
                            openvg::vg_translate(x_offset, display_height as f32 - y_offset);
                            openvg::vg_scale(scale, -scale);
                            four_up_context.draw_four(&vg_images, &styles);
                        }
                    }
                    AppState::WaitForPrint => {
                        if let Some(vg_images) = vg_images.as_ref() {
                            let (scale, x_offset, y_offset) = rocketbooth::find_center(
                                display_width as f32,
                                display_height as f32,
                                vg_images[0].width(),
                                vg_images[0].height(),
                            );
                            openvg::vg_translate(x_offset, display_height as f32 - y_offset);
                            openvg::vg_scale(scale, -scale);
                            four_up_context.draw_four(&vg_images, &[DisplayStyle::Normal; 4]);
                            openvg::vg_load_identity();

                            let next_change_time = *next_change_time.lock().unwrap();
                            let now = Instant::now();
                            let dt = {
                                let (sign, dt) = if now > next_change_time {
                                    (-1f32, now.duration_since(next_change_time))
                                } else {
                                    (1f32, next_change_time.duration_since(now))
                                };
                                sign * (dt.as_secs() as f32 + dt.subsec_nanos() as f32 * 1e-9f32)
                            };

                            let (scale, x_offset, y_offset) = rocketbooth::find_center(
                                display_width as f32,
                                display_height as f32,
                                print_card.width,
                                print_card.height,
                            );
                            let dx = (dt * std::f32::consts::FRAC_PI_2).sin()
                                * print_card.width as f32
                                / 4.0f32;
                            openvg::vg_translate(x_offset + dx, display_height as f32 - y_offset);
                            openvg::vg_scale(scale, -scale);
                            openvg::vg_draw_image(&print_image);
                        }
                    }
                }
                egl_surface.swap_buffers();
            }
        })
        .unwrap()
}

fn spawn_appstate_update_thread(
    appstate: Arc<(Mutex<AppState>, Condvar)>,
    image: Arc<Mutex<Option<raster::Image>>>,
    config: Arc<rocketbooth::PhotoboothConfig>,
    next_change_time: Arc<Mutex<Instant>>,
    print_sender: mpsc::Sender<()>,
    print_barrier: Arc<std::sync::Barrier>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("State update".to_string())
        .spawn(move || {
            // Order of timed steps and the time to spend in each state.
            let sequence = [
                (Duration::from_secs(4), AppState::Preview { index: 0 }),
                (Duration::from_secs(4), AppState::Preview { index: 1 }),
                (Duration::from_secs(4), AppState::Preview { index: 2 }),
                (Duration::from_secs(4), AppState::Preview { index: 3 }),
                (Duration::from_secs(4), AppState::WaitForPrint),
            ];
            loop {
                {
                    let lock = appstate.0.lock().unwrap();
                    let mut lock = appstate.1.wait(lock).unwrap();
                    if *lock != AppState::Ready {
                        continue;
                    }
                    *lock = AppState::Welcome;
                }
                let _video_worker = rocketbooth::VideoDecoder::new(Arc::clone(&config), Arc::clone(&image));
                {
                    let guard = appstate.0.lock().unwrap();
                    let (mut guard, timeout_info) = appstate
                        .1
                        .wait_timeout(guard, Duration::from_secs(60))
                        .unwrap();
                    if timeout_info.timed_out() {
                        *guard = AppState::Ready;
                        continue;
                    }
                };
                for &(time_delta, step) in sequence.iter() {
                    *appstate.0.lock().unwrap() = step;
                    *next_change_time.lock().unwrap() = Instant::now() + time_delta;
                    if step == AppState::WaitForPrint {
                        print_sender.send(()).unwrap();
                    }
                    if time_delta != Duration::from_secs(0) {
                        sleep(time_delta);
                    }
                }
                print_barrier.wait();
                *appstate.0.lock().unwrap() = AppState::Ready;
            }
        })
        .unwrap()
}

fn spawn_printer_thread(
    config: Arc<rocketbooth::PhotoboothConfig>,
    captures: Arc<Mutex<Option<Vec<Image>>>>,
    receiver: mpsc::Receiver<()>,
    print_barrier: Arc<std::sync::Barrier>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("Print manager".to_string())
        .spawn(move || loop {
            let _ = receiver.recv().unwrap();
            let now = ::time::now();
            let canvas = {
                let captures_guard = captures.lock().unwrap();
                let captures = captures_guard.as_ref().unwrap();
                let mut canvas =
                    rocketbooth::allocate_image(2 * captures[0].width, 2 * captures[0].height);
                canvas = blend(
                    &canvas,
                    &captures[0],
                    BlendMode::Normal,
                    1.0,
                    PositionMode::TopLeft,
                    0,
                    0,
                ).unwrap();
                canvas = blend(
                    &canvas,
                    &captures[1],
                    BlendMode::Normal,
                    1.0,
                    PositionMode::TopRight,
                    0,
                    0,
                ).unwrap();
                canvas = blend(
                    &canvas,
                    &captures[2],
                    BlendMode::Normal,
                    1.0,
                    PositionMode::BottomLeft,
                    0,
                    0,
                ).unwrap();
                canvas = blend(
                    &canvas,
                    &captures[3],
                    BlendMode::Normal,
                    1.0,
                    PositionMode::BottomRight,
                    0,
                    0,
                ).unwrap();
                canvas
            };
            let fname = format!("{}{}.jpg", config.image_prefix(), now.rfc3339());
            save(&canvas, &fname).unwrap();
            if config.enable_printing() {
                print_file_to_default_printer(&fname);
                sleep(Duration::from_secs(30));
                print_barrier.wait();
                sleep(Duration::from_secs(45));
            } else {
                print_barrier.wait();
            }
        })
        .unwrap()
}

fn load_raster_and_image(file: &str) -> (Image, rocketbooth::openvg::VGImage) {
    let image = raster::open(file).unwrap();
    let vg_image = openvg::VGImage::new(
        openvg::sys::VGImageFormat_VG_sABGR_8888,
        image.width,
        image.height,
        openvg::sys::VGImageQuality_VG_IMAGE_QUALITY_FASTER,
    );
    vg_image.sub_data(
        &image.bytes,
        4 * image.width,
        openvg::sys::VGImageFormat_VG_sABGR_8888,
        0,
        0,
        image.width,
        image.height,
    );
    (image, vg_image)
}

fn draw_image_mirrored(image: &rocketbooth::openvg::VGImage) {
    let mut matrix = [0f32; 9];
    openvg::vg_get_matrix(&mut matrix);
    openvg::vg_translate(image.width() as f32, 0f32);
    openvg::vg_scale(-1f32, 1f32);
    openvg::vg_draw_image(image);
    openvg::vg_load_matrix(&matrix);
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
enum DisplayStyle<'a> {
    Normal,
    Mirrored,
    Overlay(&'a openvg::VGImage),
    MirroredOverlay(&'a openvg::VGImage),
    Faded,
}

struct FourUpContext {
    image: Option<openvg::VGImage>,
    fade_matrix: [f32; 20],
}

impl FourUpContext {
    fn new() -> FourUpContext {
        FourUpContext {
            image: None,
            fade_matrix: [
                0.3f32, 0.3f32, 0.3f32, 0f32, 0.3f32, 0.3f32, 0.3f32, 0f32, 0.3f32, 0.3f32, 0.3f32,
                0f32, 0f32, 0f32, 0f32, 1f32, 0f32, 0f32, 0f32, 0f32,
            ],
        }
    }

    fn display(&mut self, image: &openvg::VGImage, style: &DisplayStyle) {
        match style {
            &DisplayStyle::Normal => openvg::vg_draw_image(image),
            &DisplayStyle::Mirrored => draw_image_mirrored(image),
            &DisplayStyle::Overlay(overlay_image) => {
                openvg::vg_draw_image(image);
                let (scale, dx, dy) = rocketbooth::find_center(
                    image.width() as f32,
                    image.height() as f32,
                    overlay_image.width(),
                    overlay_image.height(),
                );
                openvg::vg_translate(dx, dy);
                openvg::vg_scale(scale, scale);
                openvg::vg_draw_image(overlay_image);
            }
            &DisplayStyle::MirroredOverlay(overlay_image) => {
                draw_image_mirrored(image);
                let (scale, dx, dy) = rocketbooth::find_center(
                    image.width() as f32,
                    image.height() as f32,
                    overlay_image.width(),
                    overlay_image.height(),
                );
                openvg::vg_translate(dx, dy);
                openvg::vg_scale(scale, scale);
                openvg::vg_draw_image(overlay_image);
            }
            &DisplayStyle::Faded => {
                let faded = self.image.get_or_insert_with(|| {
                    let height = image.height();
                    let width = image.width();
                    let format = image.format();
                    let quality = openvg::sys::VGImageQuality_VG_IMAGE_QUALITY_FASTER;
                    openvg::VGImage::new(format, width, height, quality)
                });
                openvg::vg_color_matrix(faded, image, &self.fade_matrix);
                openvg::vg_draw_image(&faded);
            }
        }
    }

    fn draw_four(&mut self, images: &[openvg::VGImage], styles: &[DisplayStyle]) {
        if images.len() == 0 {
            return;
        }

        let w = images[0].width() as f32;
        let h = images[0].height() as f32;
        let positions = [(0f32, 0f32), (w, 0f32), (0f32, h), (w, h)];
        let mut matrix = [0f32; 9];

        openvg::vg_scale(0.5, 0.5);
        for ((img, sty), &(x, y)) in images.iter().zip(styles.iter()).zip(positions.into_iter()) {
            openvg::vg_get_matrix(&mut matrix);
            openvg::vg_translate(x, y);
            self.display(img, sty);
            openvg::vg_load_matrix(&matrix);
        }
    }
}

/// Send a file to the default printer configured in CUPS.
/// Returns the job ID for the created job.
fn print_file_to_default_printer(filepath: &str) -> i32 {
    use std::ptr;
    use std::ffi::CString;
    use cups_sys::*;
    unsafe {
        let mut dests: *mut cups_dest_t = ptr::null_mut();
        let num_dests = cupsGetDests(&mut dests as *mut _);
        // Get the default printer.
        let destination: *mut cups_dest_s = cupsGetDest(ptr::null(), ptr::null(), num_dests, dests);
        // Print a real page.
        let job_id: i32 = cupsPrintFile(
            (*destination).name,
            // File to print.
            CString::new(filepath).unwrap().as_ptr(),
            // Name of the print job.
            CString::new(format!("rocketbooth photo {}", filepath))
                .unwrap()
                .as_ptr(),
            (*destination).num_options,
            (*destination).options,
        );
        cupsFreeDests(num_dests, dests);
        job_id
    }
}

#[allow(dead_code)]
fn count_active_print_jobs() -> i32 {
    use std::ptr;
    use cups_sys::*;
    use cups_sys::ipp_jstate_t::*;

    unsafe {
        let mut jobs: *mut cups_job_t = ptr::null_mut();
        let num_dests = cupsGetJobs2(
            ptr::null_mut(),
            &mut jobs,
            ptr::null(),
            0,
            CUPS_WHICHJOBS_ALL as i32,
        );
        let count = (0..num_dests)
            .filter(|&i| {
                let job = &*jobs.offset(i as isize);
                let s = job.state;
                !(s == IPP_JSTATE_CANCELED || s == IPP_JSTATE_ABORTED || s == IPP_JSTATE_COMPLETED)
            })
            .count();
        cupsFreeJobs(num_dests, jobs);
        count as i32
    }
}

#[allow(dead_code)]
fn get_job_status(job_id: i32) -> Option<cups_sys::ipp_jstate_t> {
    use std::ptr;
    use cups_sys::*;

    unsafe {
        let mut jobs: *mut cups_job_t = ptr::null_mut();
        let num_dests = cupsGetJobs2(
            ptr::null_mut(),
            &mut jobs,
            ptr::null(),
            0,
            CUPS_WHICHJOBS_ALL as i32,
        );
        let mut state = None;
        for i in 0..num_dests {
            let job = &*jobs.offset(i as isize);
            if job.id == job_id {
                state = Some(job.state)
            }
        }
        cupsFreeJobs(num_dests, jobs);
        state
    }
}
