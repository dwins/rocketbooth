use std::{
    error::Error,
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use crate::{config::Config, image_sdl2::image_to_texture};
use image::RgbaImage;
use sdl2::render::{Texture, TextureCreator};

pub struct ContextBuilder {
    config: Config,
    path: PathBuf,
    prompts: [RgbaImage; 7],
}

impl ContextBuilder {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let f = File::open(path.as_ref())?;
        let mut f = BufReader::new(f);
        let mut buf = String::new();
        f.read_to_string(&mut buf)?;
        let config = toml::from_str(buf.as_str())?;
        let mut prompt_root = PathBuf::from(path.as_ref());
        prompt_root.pop();
        let load_image = |s: &str| -> Result<RgbaImage, Box<dyn Error>> {
            use image::io::Reader;
            Ok(Reader::open(prompt_root.join(s))?.decode()?.into_rgba8())
        };
        let prompts = [
            load_image("prompts/prompts.001.png")?,
            load_image("prompts/prompts.002.png")?,
            load_image("prompts/prompts.003.png")?,
            load_image("prompts/prompts.004.png")?,
            load_image("prompts/prompts.005.png")?,
            load_image("prompts/prompts.006.png")?,
            load_image("prompts/prompts.007.png")?,
        ];

        Ok(Self {
            config,
            path: path.as_ref().into(),
            prompts,
        })
    }

    pub fn build<T>(self, texture_creator: &TextureCreator<T>) -> crate::Result<Context<T>> {
        let Self {
            config,
            path,
            prompts: [prompt01, prompt02, prompt03, prompt04, prompt05, prompt06, prompt07],
        } = self;
        Ok(Context {
            config,
            path,
            texture_creator,
            prompt01: image_to_texture(prompt01, texture_creator)?,
            prompt02: image_to_texture(prompt02, texture_creator)?,
            prompt03: image_to_texture(prompt03, texture_creator)?,
            prompt04: image_to_texture(prompt04, texture_creator)?,
            prompt05: image_to_texture(prompt05, texture_creator)?,
            prompt06: image_to_texture(prompt06, texture_creator)?,
            prompt07: image_to_texture(prompt07, texture_creator)?,
        })
    }
}

pub struct Context<'t, T> {
    pub config: Config,
    pub path: PathBuf,
    pub texture_creator: &'t TextureCreator<T>,
    pub prompt01: Texture<'t>,
    pub prompt02: Texture<'t>,
    pub prompt03: Texture<'t>,
    pub prompt04: Texture<'t>,
    pub prompt05: Texture<'t>,
    pub prompt06: Texture<'t>,
    pub prompt07: Texture<'t>,
}
