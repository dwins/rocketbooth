use std::ptr::{
    null,
    null_mut,
};

use std::ffi::CString;

use av::sys::*;
use av::util::{
    Buffer,
    Dictionary,
    Packet,
};

pub use av::sys::AVFrame;

#[allow(unused)]
pub fn register_all() {
    unsafe {
        av_register_all();
        avdevice_register_all();
    }
}

pub struct Format(*mut AVInputFormat);
impl Format {
    pub fn lookup(name: &str) -> Option<Format> {
        let name_c = CString::new(name).unwrap();
        let format_ptr = unsafe {
            av_find_input_format(name_c.as_ptr())
        };
        if format_ptr.is_null() {
            Option::None
        } else {
            Option::Some(Format(format_ptr))
        }
    }
}

pub struct Frame(pub *mut AVFrame);

unsafe impl Send for Frame {} 

impl Frame {
    pub fn new() -> Option<Frame> {
        let ptr = unsafe { av_frame_alloc() };
        if ptr.is_null() {
            Option::None
        } else {
            Option::Some(Frame(ptr))
        }
    }

    pub fn borrow_avframe(&self) -> &AVFrame {
        unsafe { 
            &*self.0
        }
    }

    pub fn get_best_effort_timestamp(&self) -> i64 {
        unsafe {
            av_frame_get_best_effort_timestamp(self.0)
        }
    }

    pub fn get_linesize(&self, dimension: usize) -> i32 {
        self.borrow_avframe().linesize[dimension]
    }

    pub fn borrow_data(&mut self) -> &mut *const u8 {
        unsafe {
            ::std::mem::transmute(&mut (*self.0).data)
        }
    }

    pub fn borrow_linesize(&mut self) -> &mut [i32] {
        unsafe {
            &mut (*self.0).linesize
        }
    }

    pub fn get_channel_value(&self, dimension: usize, offset: isize) -> u8 {
        unsafe {
            *(*self.0).data[dimension].offset(offset)
        }
    }

    pub fn fill(&self, 
        buffer: &Buffer,
        format: AVPixelFormat,
        width: i32,
        height: i32)
    {
        unsafe {
            avpicture_fill(
                self.0 as *mut AVPicture,
                buffer.0 as *mut u8,
                format,
                width,
                height,
            );
        }
    }

    pub fn release(&mut self) {
        unsafe {
            av_frame_unref(self.0)
        }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe {
            av_frame_free(&mut self.0)
        }
    }
}

pub struct FormatContext(*mut AVFormatContext);

/// I promise it's ok to access the same FormatContext from multiple threads.
unsafe impl Send for FormatContext {}

impl FormatContext {
    pub fn open_input(filename: &str, options: &mut Dictionary) -> Option<FormatContext> {
        unsafe {
            let mut context = null_mut();
            let filename = CString::new(filename).unwrap();
            let status = avformat_open_input(
                &mut context,
                filename.as_ptr(),
                null_mut(),
                &mut options.0);
            if status == 0 {
                Some(FormatContext(context))
            } else {
                None
            }
        }
    }

    pub fn open_format(path: &str, format: &Format, options: &mut Dictionary) -> Option<FormatContext> {
        unsafe {
            let mut context = null_mut();
            let path_c = CString::new(path).unwrap();
            let status = avformat_open_input(
                &mut context,
                path_c.as_ptr(),
                format.0,
                &mut options.0);
            if status == 0 {
                Some(FormatContext(context))
            } else {
                None
            }
        }
    }

    pub fn borrow_format_context(&self) -> &AVFormatContext {
        unsafe { &* self.0 }
    }

    pub fn find_stream_info(&mut self) -> Option<()> {
        let status = unsafe {
            avformat_find_stream_info(self.0, null_mut())
        };
        if status >= 0 {
            Option::Some(())
        } else {
            Option::None
        }
    }

    pub fn get_streams(&self) -> &[&AVStream] {
        unsafe {
            let ctx = &*self.0;
            ::std::slice::from_raw_parts(
                ::std::mem::transmute(ctx.streams), 
                ctx.nb_streams as usize)
        }
    }

    pub fn get_stream(&self, index: usize) -> &AVStream {
        unsafe {
            let ctx = &*self.0;
            let ptr = ctx.streams.offset(index as isize);
            &**ptr
        }
    }

    pub fn dump(&mut self) {
        unsafe {
            av_dump_format(self.0, 0, null(), 0);
        }
    }

    pub fn read_into(&self, packet: &Packet) -> bool {
        let status = unsafe {
            av_read_frame(self.0, packet.0)
        };
        status >= 0
    }

    pub fn mk_packets(&mut self) -> Packets {
        unsafe {
            Packets {
                context: &mut *self.0,
                packet: av_packet_alloc(),
            }
        }
    }
}

impl Drop for FormatContext {
    fn drop(&mut self) {
        unsafe {
            avformat_close_input(&mut self.0)
        }
    }
}

impl AVStream {
    pub fn codec_ref(&self) -> &AVCodecContext {
        unsafe {
            &*self.codec
        }
    }
}

pub struct Packets<'a> {
    pub context: &'a mut AVFormatContext,
    pub packet: *mut AVPacket,
}

impl <'a> Packets<'a> {
    pub fn next(&mut self) -> Option<&AVPacket> {
        let status = unsafe {
            av_read_frame(self.context, self.packet)
        };
        if status >= 0 {
            unsafe {
                Option::Some(&*self.packet)
            }
        } else {
            Option::None
        }
    }
}
