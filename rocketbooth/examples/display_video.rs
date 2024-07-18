use rocketbooth::{frame_to_texture, FrameTextureUpdater};
use rocketbooth_libav::{FormatContext, Frame, Packet, ReceiveResult};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::render::Texture;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::thread::Thread;
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
    let mut texture: Option<(FrameTextureUpdater, Texture)> = None;

    let (sender, receiver) = sync_channel::<Frame>(0);

    let video_handle = std::thread::spawn(move || {
        let _ = read_video_frames(sender);
    });

    canvas.clear();
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
        if let Some((_, ref texture)) = texture {
            canvas.copy(texture, None, None)?;
        }
        canvas.present();

        if let Ok(frame) = receiver.try_recv() {
            match texture.as_mut() {
                Some((updater, texture)) => updater.update(&frame, texture)?,
                None => texture = frame_to_texture(&frame, &texture_creator).ok(),
            }
        }
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    drop(receiver);
    video_handle.join().ok().ok_or("Failed to join thread")?;

    Ok(())
}

fn read_video_frames(sender: SyncSender<Frame>) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = FormatContext::open(
        "/mnt/c/Users/cdwin/Downloads/VID_20171212_211842.mp4",
        None,
        None,
    )
    .ok_or("Failed to open file")?;
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
