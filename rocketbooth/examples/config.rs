use rocketbooth::{Config, ImageLayout, ImageSettings, PrintSettings, VideoSource};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config {
        video_source: VideoSource {
            path: "/dev/video0".to_string(),
            video_codec: None,
            display_size: None,
            format: None,
            options: Default::default(),
        },
        image: Some(ImageSettings {
            prefix: None,
            format: None,
            layout: ImageLayout::default(),
        }),
        print: Some(PrintSettings { enabled: true }),
    };
    let serialized = &toml::to_string(&config)?;
    println!("{serialized}");
    println!("-----");
    let config2: Config = toml::from_str(serialized)?;
    println!("{config2:#?}");
    Ok(())
}
