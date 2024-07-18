use sdl2::{
    event::Event,
    keyboard::Keycode,
    mouse::MouseButton,
    pixels::Color,
    render::{Canvas, RenderTarget, Texture, TextureCreator},
};

use crate::{image_sdl2::image_to_texture, libav_sdl2::FrameTextureManager};

pub enum State<'t, T> {
    Waiting,
    Welcome,
    Explainer(FrameTextureManager<'t, T>),
    Capture(FrameTextureManager<'t, T>),
    Debrief,
}

impl<'t, T> Default for State<'t, T> {
    fn default() -> Self {
        Self::Waiting
    }
}

impl<'t, T> State<'t, T> {
    pub fn handle_event(
        self,
        event: Event,
        context: &mut Context<'t, T>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        match event {
            Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => return Result::Err("Shutdown".into()),
            Event::MouseButtonDown {
                mouse_btn: MouseButton::Left,
                ..
            } => {
                return Ok(match self {
                    State::Waiting => State::Welcome,
                    State::Welcome => State::Explainer(FrameTextureManager::new(
                        "/mnt/c/Users/cdwin/Downloads/VID_20171212_211842.mp4",
                        context.texture_creator,
                    )?),
                    State::Explainer(texture_manager) => State::Capture(texture_manager),
                    State::Capture(_) => State::Debrief,
                    State::Debrief => State::Waiting,
                })
            }
            _ => {}
        }
        Ok(self)
    }

    pub fn render<U>(
        &mut self,
        canvas: &mut Canvas<U>,
        context: &mut Context<T>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        U: RenderTarget,
    {
        match self {
            State::Waiting => {
                canvas.set_draw_color(Color::BLACK);
                canvas.clear();
                canvas.present();
            }
            State::Welcome => {
                canvas.clear();
                canvas.copy(&context.prompt01, None, None)?;
                canvas.present();
            }
            State::Explainer(frame_texture_manager) => {
                canvas.clear();
                if let Some(texture) = frame_texture_manager.texture_ref() {
                    canvas.copy(texture, None, None)?;
                }
                canvas.copy(&context.prompt02, None, None)?;
                canvas.present();
            }
            State::Capture(frame_texture_manager) => {
                canvas.clear();
                if let Some(texture) = frame_texture_manager.texture_ref() {
                    canvas.copy(texture, None, None)?;
                }
                canvas.copy(&context.prompt03, None, None)?;
                canvas.present();
            }
            State::Debrief => {
                canvas.clear();
                canvas.copy(&context.prompt04, None, None)?;
                canvas.present();
            }
        }
        Ok(())
    }
}

pub struct Context<'t, T> {
    texture_creator: &'t TextureCreator<T>,
    prompt01: Texture<'t>,
    prompt02: Texture<'t>,
    prompt03: Texture<'t>,
    prompt04: Texture<'t>,
}

impl<'t, T> Context<'t, T> {
    pub fn new(texture_creator: &'t TextureCreator<T>) -> Result<Self, Box<dyn std::error::Error>> {
        let prompt01 = image_to_texture("./prompts/prompts.001.png", texture_creator)?;
        let prompt02 = image_to_texture("./prompts/prompts.002.png", texture_creator)?;
        let prompt03 = image_to_texture("./prompts/prompts.003.png", texture_creator)?;
        let prompt04 = image_to_texture("./prompts/prompts.004.png", texture_creator)?;
        Ok(Self {
            texture_creator,
            prompt01,
            prompt02,
            prompt03,
            prompt04,
        })
    }
}
