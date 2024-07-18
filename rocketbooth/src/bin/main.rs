use rocketbooth::{Context, State};
use std::time::Duration;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem
        .window("rust-sdl2 demo", 800, 600)
        .position_centered()
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
            state.handle_event(event)?;
        }
        state.render(&mut canvas, &mut context)?;
        // The rest of the game loop goes here...

        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
