use std::ffi::CString;
use std::ptr::null_mut;

use av::sys::{
    AVDictionary,
    AVPixelFormat,
    avpicture_get_size,
    av_dict_free,
    av_dict_set,
    av_free,
    av_malloc,
    av_packet_alloc,
    av_packet_free,
    av_packet_unref,
    av_samples_get_buffer_size,
    avpicture_fill,
};

use av::format::Frame;
pub use av::sys::AVMediaType;
pub use av::sys::AVPacket;
pub use av::sys::AVSampleFormat;

pub struct Buffer(pub *mut ::std::os::raw::c_void);

impl Buffer {
    pub fn new(size: usize) -> Option<Buffer> {
        unsafe {
            let ptr = av_malloc(size);
            if !ptr.is_null() {
                Option::Some(Buffer(ptr))
            } else {
                Option::None
            }
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            av_free(self.0)
        }
    }
}

pub use av::sys::AVPicture;

impl AVPicture {
    pub fn wrap(format: AVPixelFormat, buf: &mut [u8], width: i32, height: i32) -> AVPicture {
        let mut picture = AVPicture {
            data: [null_mut(); 8],
            linesize: [0;8],
        };
        unsafe {
            avpicture_fill(&mut picture, buf.as_mut_ptr(), format, width, height);
        };
        picture
    }

    pub fn get_size(format: AVPixelFormat, width: i32, height: i32) -> i32 {
        unsafe {
            avpicture_get_size(format, width, height)
        }
    }
}

pub struct PacketToFrameQueue {
    packets: ::std::collections::VecDeque<AVPacket>
}

impl PacketToFrameQueue {
    pub fn new() -> PacketToFrameQueue {
        PacketToFrameQueue {
            packets: ::std::collections::VecDeque::default()
        }
    }

    pub fn feed(&mut self, packet: &AVPacket) {
        self.packets.push_back(*packet)
    }

    pub fn front(&mut self) -> Option<Frame> {
        panic!()
    }
}

pub fn get_audio_buffer_size(nb_channels: i32, nb_samples: i32, format: AVSampleFormat, align: i32) -> i32 {
    unsafe {
        av_samples_get_buffer_size(
            null_mut(),
            nb_channels as ::std::os::raw::c_int,
            nb_samples as ::std::os::raw::c_int,
            format,
            align)
    }
}

pub struct Packet(pub *mut AVPacket);

impl Packet {
    pub fn new() -> Option<Packet> {
        let ptr = unsafe { av_packet_alloc() };
        if ptr.is_null() {
            Option::None
        } else {
            Option::Some(Packet(ptr))
        }
    }

    pub fn avpacket_ref(&self) -> &AVPacket {
        unsafe { &* self.0 }
    }

    pub fn release(&self) {
        unsafe {
            av_packet_unref(self.0)
        }
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        unsafe {
            av_packet_free(&mut self.0)
        }
    }
}

pub struct Dictionary(pub *mut AVDictionary);
impl Dictionary {
    pub fn new() -> Dictionary {
        Dictionary(null_mut())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        unsafe {
            let key_c = CString::new(key).unwrap();
            let value_c = CString::new(value).unwrap();
            av_dict_set(&mut self.0, key_c.as_ptr(), value_c.as_ptr(), 0);
        }
    }

    pub fn as_ref(&self) -> &AVDictionary {
        unsafe {
            &*self.0
        }
    }

    pub fn as_ref_mut(&mut self) -> &mut AVDictionary {
        unsafe {
            &mut *self.0
        }
    }
}

impl Drop for Dictionary {
    fn drop(&mut self) {
        unsafe {
            av_dict_free(&mut self.0)
        }
    }
}