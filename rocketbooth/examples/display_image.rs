use rocketbooth::image_to_texture;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use std::time::Duration;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("rust-sdl2 demo", 800, 600)
        .position_centered()
        .fullscreen()
        .build()?;

    let mut canvas = window.into_canvas().build()?;

    let texture_creator = canvas.texture_creator();
    let texture = image_to_texture("./prompts/prompts.001.png", &texture_creator)?;

    canvas.clear();
    canvas.copy(&texture, None, None)?;
    canvas.present();
    let mut event_pump = sdl_context.event_pump()?;
    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }
        // The rest of the game loop goes here...

        canvas.clear();
        canvas.copy(&texture, None, None)?;
        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
    Ok(())
}
