use std::{
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

use image::RgbImage;
use sdl2::{
    event::{Event, EventPollIterator},
    keyboard::Keycode,
    mouse::MouseButton,
    pixels::Color,
    rect::Rect,
    render::{Canvas, RenderTarget, Texture, TextureCreator},
};
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;
use time::OffsetDateTime;

use crate::{
    config::ImageLayout, image_libav::frame_to_image, image_sdl2::image_to_texture,
    libav_sdl2::FrameTextureManager, Config,
};

const FILE_TIMESTAMP_FORMAT: &[BorrowedFormatItem] =
    format_description!("[year]-[month]-[day]_[hour]:[minute]:[second]");

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
        captured_textures: Vec<Texture<'t>>,
        captured_images: Vec<RgbImage>,
        frame_texture_manager: FrameTextureManager<'t, T>,
        deadline: Instant,
    },
    Debrief {
        captured_textures: Vec<Texture<'t>>,
        deadline: Instant,
    },
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
                Event::KeyDown {
                    keycode: Some(Keycode::Return | Keycode::Space),
                    ..
                }
                | Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    ..
                } => {
                    return Ok(match self {
                        State::Waiting => State::Welcome {
                            deadline: now + Duration::from_secs(3),
                        },
                        State::Welcome { .. } => State::Explainer {
                            frame_texture_manager: FrameTextureManager::new(
                                &context.config.video_source,
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
                            captured_images: vec![],
                            captured_textures: vec![],
                        },
                        x @ State::Capture { .. } => x,
                        State::Debrief { .. } => State::Welcome {
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
                mut frame_texture_manager,
                mut captured_images,
                mut captured_textures,
            } if deadline < now => {
                let image = {
                    let frame = frame_texture_manager
                        .frame_ref()
                        .ok_or("Trying to capture before device is ready")?;
                    frame_to_image(frame)?
                };
                let texture = {
                    let texture = frame_texture_manager
                        .texture_mut()
                        .ok_or("Texture not ready yet")?;
                    let query = texture.query();
                    let mut new_texture = context.texture_creator.create_texture_static(
                        query.format,
                        query.width,
                        query.height,
                    )?;
                    std::mem::swap(texture, &mut new_texture);
                    new_texture
                };
                captured_images.push(image);
                captured_textures.push(texture);

                if captured_images.len()
                    < (context.config.image.as_ref())
                        .map_or(ImageLayout::default(), |cfg| cfg.layout)
                        .capture_count()
                {
                    State::Capture {
                        deadline: deadline + Duration::from_secs(3),
                        frame_texture_manager,
                        captured_images,
                        captured_textures,
                    }
                } else {
                    let post_command = context.config.image.as_ref().and_then(|cfg| {
                        if cfg.enable_post_command {
                            cfg.post_command.as_ref().filter(|v| !v.is_empty()).cloned()
                        } else {
                            None
                        }
                    });
                    let layout = context
                        .config
                        .image
                        .as_ref()
                        .map_or(ImageLayout::default(), |settings| settings.layout);
                    let (width, height) =
                        layout.dest_size(captured_images[0].width(), captured_images[0].height());

                    let prefix = (context.config.image.as_ref())
                        .and_then(|img| img.prefix.as_ref())
                        .map_or("", |s| s.as_str());
                    let format = (context.config.image.as_ref())
                        .and_then(|cfg| cfg.format.as_ref())
                        .map_or("", |s| s.as_str());
                    let format = if format == "PNG" {
                        image::ImageFormat::Png
                    } else {
                        image::ImageFormat::Jpeg
                    };
                    let suffix = if format == image::ImageFormat::Png {
                        "png"
                    } else {
                        "jpeg"
                    };
                    let timestamp = OffsetDateTime::now_local()
                        .unwrap_or_else(|_| OffsetDateTime::now_utc())
                        .format(FILE_TIMESTAMP_FORMAT)?;
                    let saved_path: PathBuf = format!("{prefix}img_{timestamp}.{suffix}").into();
                    std::thread::spawn(move || {
                        let mut final_image = RgbImage::new(width, height);
                        for (&(x, y, _, _), partial_image) in Iterator::zip(
                            layout.arrange_within_rect(width, height).iter(),
                            captured_images.iter(),
                        ) {
                            image::imageops::overlay(
                                &mut final_image,
                                partial_image,
                                x as i64,
                                y as i64,
                            );
                        }
                        final_image.save_with_format(&saved_path, format).unwrap();
                        if let Some(post_command) = post_command {
                            let _ = Command::new(&post_command[0])
                                .args(&post_command[1..])
                                .arg(saved_path)
                                .output()
                                .unwrap();
                        }
                    });
                    State::Debrief {
                        captured_textures,
                        deadline: deadline + Duration::from_secs(5),
                    }
                }
            }
            State::Debrief { deadline, .. } if deadline < now => State::Welcome {
                deadline: deadline + Duration::from_secs(3),
            },
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
                captured_textures,
                deadline,
                ..
            } => {
                let t = *deadline - Instant::now();
                let countdown_overlay = match t.as_secs() {
                    0 => Some(&context.prompt06),
                    1 => Some(&context.prompt05),
                    2 => Some(&context.prompt04),
                    3 => Some(&context.prompt03),
                    _ => None,
                };
                let layout = (context.config.image.as_ref())
                    .map_or(ImageLayout::default(), |cfg| cfg.layout);
                let (width, height) = canvas.output_size()?;
                let texture_iter = Iterator::chain(
                    captured_textures.iter(),
                    frame_texture_manager.texture_ref(),
                );
                canvas.clear();

                let rects = layout.arrange_within_rect(width, height);
                for (&(x, y, w, h), tex) in Iterator::zip(rects.iter(), texture_iter) {
                    canvas.copy(tex, None, Some(Rect::new(x as i32, y as i32, w, h)))?;
                }
                if let Some(overlay) = countdown_overlay {
                    let rect = rects
                        .get(captured_textures.len())
                        .map(|&(x, y, w, h)| Rect::new(x as i32, y as i32, w, h));
                    if rect.is_some() {
                        canvas.copy(overlay, None, rect)?;
                    }
                }
                canvas.present();
            }
            State::Debrief {
                captured_textures, ..
            } => {
                let layout = (context.config.image.as_ref())
                    .map_or(ImageLayout::default(), |cfg| cfg.layout);
                let (width, height) = canvas.output_size()?;
                canvas.clear();
                for (&(x, y, w, h), tex) in Iterator::zip(
                    layout.arrange_within_rect(width, height).iter(),
                    captured_textures.iter(),
                ) {
                    canvas.copy(tex, None, Some(Rect::new(x as i32, y as i32, w, h)))?;
                }
                canvas.copy(&context.prompt07, None, None)?;
                canvas.present();
            }
        }
        Ok(())
    }
}

pub struct Context<'t, T> {
    config: Config,
    texture_creator: &'t TextureCreator<T>,
    prompt01: Texture<'t>,
    prompt02: Texture<'t>,
    prompt03: Texture<'t>,
    prompt04: Texture<'t>,
    prompt05: Texture<'t>,
    prompt06: Texture<'t>,
    prompt07: Texture<'t>,
}

impl<'t, T> Context<'t, T> {
    pub fn new(
        config: Config,
        texture_creator: &'t TextureCreator<T>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let prompt01 = image_to_texture("./prompts/prompts.001.png", texture_creator)?;
        let prompt02 = image_to_texture("./prompts/prompts.002.png", texture_creator)?;
        let prompt03 = image_to_texture("./prompts/prompts.003.png", texture_creator)?;
        let prompt04 = image_to_texture("./prompts/prompts.004.png", texture_creator)?;
        let prompt05 = image_to_texture("./prompts/prompts.005.png", texture_creator)?;
        let prompt06 = image_to_texture("./prompts/prompts.006.png", texture_creator)?;
        let prompt07 = image_to_texture("./prompts/prompts.007.png", texture_creator)?;
        Ok(Self {
            config,
            texture_creator,
            prompt01,
            prompt02,
            prompt03,
            prompt04,
            prompt05,
            prompt06,
            prompt07,
        })
    }
}
