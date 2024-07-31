use core::slice;
use std::{
    ffi::{CStr, CString, NulError},
    ptr::null_mut,
};

use libc::{EAGAIN, EINVAL};
use sys::{
    av_dict_free, av_dict_set, av_find_input_format, av_frame_alloc, av_frame_free,
    av_frame_get_buffer, av_free, av_get_padded_bits_per_pixel, av_malloc, av_packet_alloc,
    av_packet_free, av_packet_unref, av_pix_fmt_desc_get, av_read_frame, avcodec_alloc_context3,
    avcodec_find_decoder, avcodec_find_decoder_by_name, avcodec_free_context, avcodec_open2,
    avcodec_parameters_to_context, avcodec_receive_frame, avcodec_send_packet,
    avdevice_register_all, avformat_close_input, avformat_find_stream_info, avformat_open_input,
    sws_freeContext, sws_getContext, sws_scale, AVCodecContext, AVDictionary, AVFormatContext,
    AVFrame, AVInputFormat, AVMediaType, AVPacket, AVPixelFormat, AVStream, SwsContext,
    AV_PIX_FMT_FLAG_RGB, SWS_FAST_BILINEAR,
};

mod sys;
pub struct Dictionary(*mut AVDictionary);

unsafe impl Send for Dictionary {}

impl Dictionary {
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), NulError> {
        let key = CString::new(key)?;
        let value = CString::new(value)?;
        unsafe {
            av_dict_set(&mut self.0, key.as_ptr(), value.as_ptr(), 0);
        }
        Ok(())
    }
}

impl<S0, S1, T> From<T> for Dictionary
where
    S0: AsRef<str>,
    S1: AsRef<str>,
    T: IntoIterator<Item = (S0, S1)>,
{
    fn from(value: T) -> Self {
        let mut result = Dictionary::default();
        for (key, value) in value {
            result.insert(key.as_ref(), value.as_ref());
        }
        result
    }
}

impl Default for Dictionary {
    fn default() -> Self {
        Dictionary(null_mut())
    }
}

impl Drop for Dictionary {
    fn drop(&mut self) {
        unsafe { av_dict_free(&mut self.0) }
    }
}

impl Dictionary {
    pub fn insert(&mut self, key: &str, value: &str) {
        #[allow(temporary_cstring_as_ptr)]
        unsafe {
            av_dict_set(
                &mut self.0,
                CString::new(key).unwrap().as_ptr(),
                CString::new(value).unwrap().as_ptr(),
                0,
            );
        }
    }
}

pub struct Packet(*mut AVPacket);

impl Packet {
    pub fn new() -> Option<Self> {
        Some(unsafe { av_packet_alloc() })
            .filter(|ptr| !ptr.is_null())
            .map(Packet)
    }

    pub fn stream_index(&self) -> i32 {
        unsafe { *self.0 }.stream_index
    }

    pub fn decrement_ref_count(&self) {
        unsafe { av_packet_unref(self.0) }
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        unsafe { av_packet_free(&mut self.0) }
    }
}

pub struct Buffer(*mut std::os::raw::c_void);

impl Buffer {
    pub fn new(size: usize) -> Option<Self> {
        Some(unsafe { av_malloc(size) })
            .filter(|ptr| !ptr.is_null())
            .map(Self)
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe { av_free(self.0) }
    }
}

static FORMAT_INIT: std::sync::Once = std::sync::Once::new();

pub struct Format(*mut AVInputFormat);

unsafe impl Send for Format {}

impl Format {
    pub fn from_name(name: &str) -> Option<Self> {
        FORMAT_INIT.call_once(|| unsafe {
            avdevice_register_all();
        });
        let name = CString::new(name).ok()?;
        Some(unsafe { av_find_input_format(name.as_ptr()) })
            .filter(|ptr| !ptr.is_null())
            .map(|ptr| Self(ptr as _))
    }
}

pub struct FormatContext(*mut AVFormatContext);

impl FormatContext {
    pub fn open(
        path: &str,
        format: Option<Format>,
        mut options: Option<Dictionary>,
    ) -> Option<FormatContext> {
        let mut context = null_mut();
        let path = CString::new(path).ok()?;
        let format = format.map_or(null_mut(), |fmt| fmt.0);
        let mut fallback_options = null_mut();
        let options = options
            .as_mut()
            .map_or(&mut fallback_options, |dict| &mut dict.0);
        let status = unsafe { avformat_open_input(&mut context, path.as_ptr(), format, options) };
        if status == 0 {
            Some(FormatContext(context))
        } else {
            None
        }
    }

    pub fn find_stream_info(&mut self) {
        let status = unsafe { avformat_find_stream_info(self.0, null_mut()) };
        assert!(status >= 0);
    }

    pub fn streams(&mut self) -> impl Iterator<Item = Stream> + '_ {
        let as_slice =
            unsafe { std::slice::from_raw_parts((*self.0).streams, (*self.0).nb_streams as usize) };
        as_slice.iter().map(|&av_stream| Stream(av_stream))
    }

    pub fn read_into(&mut self, packet: &mut Packet) -> bool {
        0 == unsafe { av_read_frame(self.0, packet.0) }
    }
}

impl Drop for FormatContext {
    fn drop(&mut self) {
        unsafe {
            avformat_close_input(&mut self.0);
        }
    }
}

pub struct Frame(*mut AVFrame);

unsafe impl Send for Frame {}

impl Frame {
    pub fn new() -> Option<Self> {
        Some(unsafe { av_frame_alloc() })
            .filter(|ptr| !ptr.is_null())
            .map(Self)
    }

    fn alloc(width: i32, height: i32, format: AVPixelFormat) -> Option<Self> {
        let ptr = unsafe { av_frame_alloc() };
        if ptr.is_null() {
            return None;
        }
        unsafe {
            (*ptr).width = width;
            (*ptr).height = height;
            (*ptr).format = format.0;
        }
        if unsafe { av_frame_get_buffer(ptr, 0) } != 0 {
            return None;
        };
        Some(Self(ptr))
    }

    pub fn alloc_rgb24(width: i32, height: i32) -> Option<Self> {
        Self::alloc(width, height, AVPixelFormat::AV_PIX_FMT_RGB24)
    }

    pub fn alloc_yuv420p(width: i32, height: i32) -> Option<Self> {
        Self::alloc(width, height, AVPixelFormat::AV_PIX_FMT_YUV420P)
    }

    pub fn id(&self) -> i64 {
        unsafe { *(self.0) }.pkt_pos
    }

    pub fn is_yuv420p(&self) -> bool {
        self.format() == AVPixelFormat::AV_PIX_FMT_YUV420P
    }

    pub fn is_rgb24(&self) -> bool {
        self.format() == AVPixelFormat::AV_PIX_FMT_RGB24
    }

    pub fn is_any_rgb_format(&self) -> bool {
        let pix_desc = unsafe { &*av_pix_fmt_desc_get(self.format()) };
        pix_desc.flags & AV_PIX_FMT_FLAG_RGB as u64 != 0
    }

    pub fn format(&self) -> AVPixelFormat {
        AVPixelFormat(unsafe { *self.0 }.format)
    }

    pub fn format_name(&self) -> String {
        let pix_desc = unsafe { av_pix_fmt_desc_get(self.format()) };
        unsafe {
            CStr::from_ptr((*pix_desc).name)
                .to_string_lossy()
                .into_owned()
        }
    }

    pub fn height(&self) -> usize {
        unsafe { *self.0 }.height as _
    }

    pub fn width(&self) -> usize {
        unsafe { *self.0 }.width as _
    }

    pub fn pitch(&self) -> usize {
        let pix_desc = unsafe { av_pix_fmt_desc_get(self.format()) };
        unsafe { av_get_padded_bits_per_pixel(pix_desc) as usize * self.width() / 8 }
    }

    pub fn samples(&self) -> &[u8] {
        unsafe {
            let ptr = (*self.0).data[0];
            let pix_desc = av_pix_fmt_desc_get(self.format());
            let bits_per_pixel = av_get_padded_bits_per_pixel(pix_desc);
            slice::from_raw_parts(
                ptr,
                self.width() * self.height() * bits_per_pixel as usize / 8,
            )
        }
    }

    pub fn yuv_samples(&self) -> YuvSamples {
        unsafe {
            let data = &(*self.0).data;
            let linesize = &(*self.0).linesize;
            let height = self.height();
            YuvSamples {
                y_samples: slice::from_raw_parts(data[0], linesize[0] as usize * height),
                y_pitch: linesize[0] as usize,
                u_samples: slice::from_raw_parts(data[1], linesize[1] as usize * height / 2),
                u_pitch: linesize[1] as usize,
                v_samples: slice::from_raw_parts(data[2], linesize[2] as usize * height / 2),
                v_pitch: linesize[2] as usize,
            }
        }
    }
}

pub struct YuvSamples<'t> {
    pub y_samples: &'t [u8],
    pub y_pitch: usize,
    pub u_samples: &'t [u8],
    pub u_pitch: usize,
    pub v_samples: &'t [u8],
    pub v_pitch: usize,
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe { av_frame_free(&mut self.0) }
    }
}

pub struct Stream(*mut AVStream);

impl Stream {
    pub fn index(&self) -> i32 {
        unsafe { (*self.0).index }
    }

    pub fn is_video(&self) -> bool {
        let codec_type = unsafe { (*(*self.0).codecpar).codec_type };
        codec_type == AVMediaType::AVMEDIA_TYPE_VIDEO
    }

    pub fn create_decoder(&self, name: Option<&str>) -> Option<Decoder> {
        let borrowed_codec = unsafe { (*self.0).codecpar };
        let codec = if let Some(name) = name {
            unsafe {
                let name = CString::new(name).unwrap();
                avcodec_find_decoder_by_name(name.as_ptr())
            }
        } else {
            unsafe { avcodec_find_decoder((*borrowed_codec).codec_id) }
        };
        if codec.is_null() {
            return None;
        }
        let decoder = unsafe { avcodec_alloc_context3(codec) };
        let status = unsafe { avcodec_parameters_to_context(decoder, borrowed_codec) };
        if status != 0 {
            return None;
        }
        unsafe { avcodec_open2(decoder, codec, null_mut()) };
        Some(Decoder { decoder })
    }
}

#[derive(Debug)]
pub enum ReceiveResult {
    Success,
    Error,
    Pending,
    Done,
}

pub struct Decoder {
    decoder: *mut AVCodecContext,
}

impl Decoder {
    pub fn send(&mut self, packet: &mut Packet) -> bool {
        0 == unsafe { -avcodec_send_packet(self.decoder, packet.0) }
    }

    pub fn send_eof(&mut self) -> bool {
        0 == unsafe { avcodec_send_packet(self.decoder, null_mut()) }
    }

    pub fn receive(&mut self, frame: &mut Frame) -> ReceiveResult {
        let status = unsafe { avcodec_receive_frame(self.decoder, frame.0) };
        match -status {
            0 => ReceiveResult::Success,
            EAGAIN => ReceiveResult::Pending,
            EINVAL => ReceiveResult::Error,
            _ => ReceiveResult::Done,
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            avcodec_free_context(&mut self.decoder);
        }
    }
}

pub struct ScalingContext(*mut SwsContext);

impl ScalingContext {
    pub fn new(
        src_width: i32,
        src_height: i32,
        src_format: AVPixelFormat,
        dst_width: i32,
        dst_height: i32,
        dst_format: AVPixelFormat,
    ) -> Self {
        let context = unsafe {
            sws_getContext(
                src_width,
                src_height,
                src_format,
                dst_width,
                dst_height,
                dst_format,
                SWS_FAST_BILINEAR as _,
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        ScalingContext(context)
    }

    pub fn scale(&mut self, src: &Frame, dest: &mut Frame) {
        unsafe {
            let src_slice = (*src.0).data.as_ptr().cast::<*const u8>();
            let src_stride = (*src.0).linesize.as_ptr();
            let dst_slice = (*dest.0).data.as_ptr();
            let dst_stride = (*dest.0).linesize.as_ptr();
            sws_scale(
                self.0,
                src_slice,
                src_stride,
                0,
                src.height() as _,
                dst_slice,
                dst_stride,
            );
        }
    }
}

impl Drop for ScalingContext {
    fn drop(&mut self) {
        unsafe {
            sws_freeContext(self.0);
        }
    }
}
