use std::ptr::null_mut;

use av::sys::*;
use av::format::{
    Frame,
};
use av::util::Packet;

pub use av::sys::AVCodecID as CodecID;

pub struct CodecContext(*mut AVCodecContext, *mut AVCodec);

/// I promise it's ok to access the same CodecContext from multiple threads.
unsafe impl Send for CodecContext {}

impl CodecContext {
    pub fn create_decode_context(original: &AVCodecContext) -> Option<CodecContext> {
        unsafe {
            let codec = avcodec_find_decoder(original.codec_id);
            if codec.is_null() {
                Option::None
            } else {
                let decoder = avcodec_alloc_context3(codec);
                let status = avcodec_copy_context(decoder, original);
                if status != 0 {
                    Option::None
                } else {
                    Option::Some(CodecContext(decoder, codec))
                }
            }
        }
    }

    pub fn open(&mut self) -> Option<()> {
        unsafe {
            let status = avcodec_open2(self.0, self.1, null_mut());
            if status != 0 {
                Option::None
            } else {
                Option::Some(())
            }
        }
    }

    pub fn avcodec_context_ref(&self) -> &AVCodecContext {
        unsafe {
            &*self.0
        }
    }

    pub fn get_channels(&self) -> i32 {
        self.avcodec_context_ref().channels
    }

    pub fn get_format(&self) -> AVPixelFormat {
        self.avcodec_context_ref().pix_fmt
    }

    pub fn get_height(&self) -> i32 {
        self.avcodec_context_ref().height
    }

    pub fn get_sample_rate(&self) -> i32 {
        self.avcodec_context_ref().sample_rate
    }

    pub fn get_width(&self) -> i32 {
        self.avcodec_context_ref().width
    }

    pub fn as_audio(&self) -> Option<AudioContext> {
        let ctx = unsafe { &*self.0 };
        let codec = unsafe { &*ctx.codec };
        if codec.type_ == AVMediaType_AVMEDIA_TYPE_AUDIO {
            Option::Some(AudioContext(&self))
        } else {
            Option::None
        }
    }

    pub fn as_video(&self) -> Option<VideoContext> {
        let ctx = unsafe { &*self.0 };
        let codec = unsafe { &* ctx.codec };
        if codec.type_ == AVMediaType_AVMEDIA_TYPE_VIDEO {
            Option::Some(VideoContext(&self))
        } else {
            Option::None
        }
    }

    /// Accept a packet to be decoded by this codec.
    /// Returns true iff the packet was accepted (ie, there is sufficient space internal to the codeccontext to queue the packet for decoding.)
    /// If the packet not accepted, more frames need to be received before trying again.
    pub fn send(&self, packet: &Packet) -> bool {
        unsafe {
            0 == avcodec_send_packet(self.0, packet.0)
        }
    }

    /// Notify the codec that there are no more packets to be read
    pub fn send_eof(&self) -> bool {
        unsafe {
            0 == avcodec_send_packet(self.0, null_mut())
        }
    }

    pub fn receive(&self, frame: &Frame) -> ReceiveState {
        unsafe {
            let status = avcodec_receive_frame(self.0, frame.0);
            // println!("{}", status);
            match -status {
                0 => ReceiveState::SUCCESS,
                ::libc::EAGAIN => ReceiveState::PENDING,
                ::libc::EINVAL => ReceiveState::ERROR,
                _ => ReceiveState::DONE,
            }
        }
    }
}

#[derive(Debug)]
pub enum ReceiveState {
    SUCCESS,
    PENDING,
    DONE,
    ERROR,
}

impl Drop for CodecContext {
    fn drop(&mut self) {
        unsafe {
            avcodec_close(self.0);
        }
    }
}

pub trait Decode {
    fn decode(&self, frame: &mut Frame, packet: &AVPacket) -> bool;
}

pub struct AudioContext<'c>(&'c CodecContext);

impl <'c> AudioContext<'c> {
    pub fn get_channels(&self) -> i32 {
        self.0.avcodec_context_ref().channels
    }

    pub fn get_sample_format(&self) -> AVSampleFormat {
        self.0.avcodec_context_ref().sample_fmt
    }
}

impl <'c> Decode for AudioContext<'c> {
    fn decode(&self, frame: &mut Frame, packet: &AVPacket) -> bool {
        unsafe {
            let mut got_frame = 0i32;
            avcodec_decode_audio4((self.0).0, frame.0, &mut got_frame, packet);
            got_frame != 0
        }
    }
}

pub struct VideoContext<'c>(&'c CodecContext);
impl <'c> Decode for VideoContext<'c> {
    fn decode(&self, frame: &mut Frame, packet: &AVPacket) -> bool {
        unsafe {
            let mut finished = 0i32;
            avcodec_decode_video2((self.0).0, frame.0, &mut finished, packet);
            finished != 0
        }
    }
}


pub struct Frames<'c, C : Decode + 'c> {
    pub codec_context: &'c C,
    queue: ::std::collections::VecDeque<AVPacket>,
    frame: Frame,
}

impl <'c, C : Decode + 'c> Frames<'c, C>  {
    pub fn new(codec_context: &'c C) -> Frames<'c, C> {
        Frames {
            codec_context,
            queue: ::std::default::Default::default(),
            frame: Frame::new().unwrap(),
        }
    }

    pub fn feed(&mut self, packet: &AVPacket) {
        self.queue.push_back(*packet)
    }

    pub fn poll(&mut self) -> Option<&Frame> {
        while let Option::Some(packet) = self.queue.pop_front() {
            if self.codec_context.decode(&mut self.frame, &packet) {
                return Option::Some(&self.frame);
            }
        }
        Option::None
    }
}