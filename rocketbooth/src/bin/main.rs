use rocketbooth::{Context, State};
use std::time::Duration;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    let display_mode = video_subsystem.display_mode(0, 0)?;

    let window = video_subsystem
        .window("rust-sdl2 demo", display_mode.w as _, display_mode.h as _)
        .fullscreen()
        .build()?;

    let mut canvas = window.into_canvas().build()?;

    let texture_creator = canvas.texture_creator();
    let mut context = Context::new(&texture_creator)?;
    let mut state = State::default();

    state.render(&mut canvas, &mut context)?;

    let mut event_pump = sdl_context.event_pump()?;
    loop {
        for event in event_pump.poll_iter() {
            state = state.handle_event(event, &mut context)?;
        }
        state.render(&mut canvas, &mut context)?;
        // The rest of the game loop goes here...

        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
