use std::{fs::File, io::{BufReader, Read}};

use rocketbooth::{Config, Context, State};

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: Config = {
        let f = File::open("Rocketbooth.toml")?;
        let mut f = BufReader::new(f);
        let mut buf = String::new();
        f.read_to_string(&mut buf)?;
        toml::from_str(&buf)?
    };
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    let display_mode = video_subsystem.display_mode(0, 0)?;

    let window = video_subsystem
        .window("rust-sdl2 demo", display_mode.w as _, display_mode.h as _)
        .fullscreen()
        .build()?;

    let mut canvas = window.into_canvas().accelerated().present_vsync().build()?;

    let texture_creator = canvas.texture_creator();
    let mut context = Context::new(config, &texture_creator)?;
    let mut state = State::default();

    state.render(&mut canvas, &mut context)?;

    let mut event_pump = sdl_context.event_pump()?;
    loop {
        state = state.handle_event(event_pump.poll_iter(), &mut context)?;
        state.render(&mut canvas, &mut context)?;
        // The rest of the game loop goes here...
    }
}
