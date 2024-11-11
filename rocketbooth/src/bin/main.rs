use std::env::args;

use rocketbooth::{ContextBuilder, State};

#[cfg(feature = "gpio")]
struct GpioEvent();

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let candidate_config_paths: Vec<String> = args().skip(1).collect();
    let candidate_config_paths = if candidate_config_paths.is_empty() {
        vec![String::from("Rocketbooth.toml")]
    } else {
        candidate_config_paths
    };
    let context_builder: ContextBuilder = candidate_config_paths
        .iter()
        .find_map(|path| ContextBuilder::from_file(path).ok())
        .ok_or_else(|| format!("No valid config file found; checked {candidate_config_paths:?}"))?;
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    #[cfg(feature = "gpio")]
    let _gpio_worker = {
        let ev = sdl_context.event().unwrap();
        ev.register_custom_event::<GpioEvent>().unwrap();
        let sender = ev.event_sender();
        std::thread::spawn(move || {
            use gpiochip;
            let chip = gpiochip::GpioChip::new("/dev/gpiochip0").unwrap();
            let button = chip
                .request_event(
                    "rocketbooth",
                    2,
                    gpiochip::RequestFlags::INPUT,
                    gpiochip::EventRequestFlags::RISING_EDGE,
                )
                .unwrap();
            let mut last_fired_event = 0u64;
            loop {
                let bitmap = gpiochip::wait_for_event(&[&button], 200).unwrap();
                if bitmap & 0b01 == 0b01 {
                    let event = button.read().unwrap();
                    if event.timestamp > last_fired_event + 500_000_000u64 {
                        last_fired_event = event.timestamp;
                        sender.push_custom_event(GpioEvent()).unwrap();
                    }
                }
            }
        })
    };
    let display_mode = video_subsystem.display_mode(0, 0)?;

    let window = video_subsystem
        .window("rust-sdl2 demo", display_mode.w as _, display_mode.h as _)
        .fullscreen()
        .build()?;

    let mut canvas = window.into_canvas().accelerated().present_vsync().build()?;

    let texture_creator = canvas.texture_creator();
    let mut context = context_builder.build(&texture_creator)?;
    let mut state = State::default();

    state.render(&mut canvas, &mut context)?;

    let mut event_pump = sdl_context.event_pump()?;

    sdl_context.mouse().show_cursor(false);

    loop {
        state = state.handle_event(event_pump.poll_iter(), &mut context)?;
        state.render(&mut canvas, &mut context)?;
        // The rest of the game loop goes here...
    }
}
