use std::{
    collections::HashMap,
    ops::{Div, Mul, Sub},
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct VideoSource {
    pub path: String,
    pub format: Option<String>,
    pub video_codec: Option<String>,
    pub display_size: Option<(usize, usize)>,
    #[serde(default)]
    pub options: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ImageSettings {
    #[serde(default)]
    pub layout: ImageLayout,
    pub prefix: Option<String>,
    pub format: Option<String>,
    pub post_command: Option<Vec<String>>,
    #[serde(default="default_post_command")]
    pub enable_post_command: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub video_source: VideoSource,
    pub image: Option<ImageSettings>,
}

fn default_post_command() -> bool {
    true
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImageLayout {
    #[default]
    Single,
    TwoByTwo,
}

impl ImageLayout {
    pub fn capture_count(&self) -> usize {
        match self {
            Self::Single => 1,
            Self::TwoByTwo => 4,
        }
    }

    pub fn dest_size<T>(&self, width: T, height: T) -> (T, T)
    where
        T: Copy + Mul<Output = T> + From<u8>,
    {
        match self {
            Self::Single => (width, height),
            Self::TwoByTwo => (T::from(2u8) * width, T::from(2u8) * height),
        }
    }

    pub fn arrange_within_rect<T>(&self, width: T, height: T) -> Vec<(T, T, T, T)>
    where
        T: Copy + Sub<Output = T> + Div<Output = T> + From<u8>,
    {
        let zero = 0u8.into();
        match self {
            Self::Single => vec![(zero, zero, width, height)],
            Self::TwoByTwo => {
                let w0 = width / 2u8.into();
                let w1 = width - w0;
                let h0 = height / 2u8.into();
                let h1 = height - h0;
                vec![
                    (zero, zero, w0, h0),
                    (w0, zero, w1, h0),
                    (zero, h0, w0, h1),
                    (w0, h0, w1, h1),
                ]
            }
        }
    }
}
