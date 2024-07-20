use std::time::{Duration, Instant};

use sdl2::{
    event::{Event, EventPollIterator},
    keyboard::Keycode,
    mouse::MouseButton,
    pixels::Color,
    render::{Canvas, RenderTarget, Texture, TextureCreator},
};

use crate::{
    image_libav::frame_to_image, image_sdl2::image_to_texture, libav_sdl2::FrameTextureManager,
};

pub enum State<'t, T> {
    Waiting,
    Welcome {
        deadline: Instant,
    },
    Explainer {
        frame_texture_manager: FrameTextureManager<'t, T>,
        deadline: Instant,
    },
    Capture {
        frame_texture_manager: FrameTextureManager<'t, T>,
        deadline: Instant,
    },
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
        events: EventPollIterator,
        context: &mut Context<'t, T>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let now = std::time::Instant::now();

        for event in events {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape | Keycode::Q),
                    ..
                } => return Result::Err("Shutdown".into()),
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    ..
                } => {
                    return Ok(match self {
                        State::Waiting => State::Welcome {
                            deadline: now + Duration::from_secs(3),
                        },
                        State::Welcome { .. } => State::Explainer {
                            frame_texture_manager: FrameTextureManager::new(
                                "/mnt/c/Users/cdwin/Downloads/VID_20171212_211842.mp4",
                                context.texture_creator,
                            )?,
                            deadline: now + Duration::from_secs(30),
                        },
                        State::Explainer {
                            frame_texture_manager,
                            ..
                        } => State::Capture {
                            frame_texture_manager,
                            deadline: now + Duration::from_secs(3),
                        },
                        State::Capture {
                            frame_texture_manager,
                            ..
                        } => {
                            if let Some(frame) = frame_texture_manager.frame_ref() {
                                let img = frame_to_image(frame)?;
                                img.save_with_format("./img.jpg", image::ImageFormat::Jpeg)?;
                            }
                            State::Debrief
                        }
                        State::Debrief => State::Welcome {
                            deadline: Instant::now() + Duration::from_secs(3),
                        },
                    })
                }
                _ => {}
            }
        }

        Ok(match self {
            State::Welcome { deadline } | State::Explainer { deadline, .. } if deadline < now => {
                State::Waiting
            }
            State::Capture {
                deadline,
                frame_texture_manager,
            } if deadline < now => {
                if let Some(frame) = frame_texture_manager.frame_ref() {
                    let img = frame_to_image(frame)?;
                    img.save_with_format("./img.jpg", image::ImageFormat::Jpeg)?;
                }
                State::Debrief
            }
            _ => self,
        })
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
            State::Welcome { .. } => {
                canvas.clear();
                canvas.copy(&context.prompt01, None, None)?;
                canvas.present();
            }
            State::Explainer {
                frame_texture_manager,
                ..
            } => {
                canvas.clear();
                if let Some(texture) = frame_texture_manager.texture_ref() {
                    canvas.copy(texture, None, None)?;
                }
                canvas.copy(&context.prompt02, None, None)?;
                canvas.present();
            }
            State::Capture {
                frame_texture_manager,
                ..
            } => {
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
