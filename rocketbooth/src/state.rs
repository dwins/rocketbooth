use sdl2::{
    event::Event,
    keyboard::Keycode,
    mouse::MouseButton,
    pixels::Color,
    render::{Canvas, RenderTarget, Texture, TextureCreator},
};

use crate::image_sdl2::image_to_texture;

pub enum State {
    Waiting,
    Welcome,
    Explainer,
    Capture,
    Debrief,
}

impl Default for State {
    fn default() -> Self {
        Self::Waiting
    }
}

impl State {
    pub fn handle_event(&mut self, event: Event) -> Result<(), Box<dyn std::error::Error>> {
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
                *self = match self {
                    State::Waiting => State::Welcome,
                    State::Welcome => State::Explainer,
                    State::Explainer => State::Capture,
                    State::Capture => State::Debrief,
                    State::Debrief => State::Waiting,
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn render<T>(
        &self,
        canvas: &mut Canvas<T>,
        context: &mut Context,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: RenderTarget,
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
            State::Explainer => {
                canvas.clear();
                canvas.copy(&context.prompt01, None, None)?;
                canvas.copy(&context.prompt02, None, None)?;
                canvas.present();
            }
            State::Capture => {
                canvas.clear();
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

pub struct Context<'t> {
    prompt01: Texture<'t>,
    prompt02: Texture<'t>,
    prompt03: Texture<'t>,
    prompt04: Texture<'t>,
}

impl<'t> Context<'t> {
    pub fn new<T>(
        texture_creator: &'t TextureCreator<T>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let prompt01 = image_to_texture("./prompts/prompts.001.png", texture_creator)?;
        let prompt02 = image_to_texture("./prompts/prompts.002.png", texture_creator)?;
        let prompt03 = image_to_texture("./prompts/prompts.003.png", texture_creator)?;
        let prompt04 = image_to_texture("./prompts/prompts.004.png", texture_creator)?;
        Ok(Self {
            prompt01,
            prompt02,
            prompt03,
            prompt04,
        })
    }
}
