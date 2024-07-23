use rocketbooth_libav::{FormatContext, Frame, Packet, ReceiveResult};

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        .create_decoder(None)
        .ok_or("Codec failed to initialize")?;
    println!("Reading video stream #{}", video_stream.index());
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
                    ReceiveResult::Success => print!("\rReceived a frame {}", frame.id()),
                }
            }
        }
    }
    println!();
    Ok(())
}
