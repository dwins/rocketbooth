mod config;
mod image_libav;
mod image_sdl2;
mod libav_sdl2;
mod state;

pub use config::{Config, Controls, ImageLayout, ImageSettings, VideoSource};
pub use image_sdl2::image_to_texture;
pub use libav_sdl2::{frame_to_texture, FrameTextureUpdater};
pub use state::{Context, ContextBuilder, State};

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;