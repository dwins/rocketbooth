use std::ptr::null_mut;

use av::sys::{
    AVPicture,
    AVPixelFormat,
    SwsContext,
    sws_getContext,
    sws_scale,
    sws_freeContext,
    SWS_BILINEAR,
    SWS_FAST_BILINEAR,
    SWS_BICUBIC,
    SWS_X,
    SWS_POINT,
    SWS_AREA,
    SWS_BICUBLIN,
    SWS_GAUSS,
    SWS_SINC,
    SWS_LANCZOS,
    SWS_SPLINE,
};
use av::format::Frame;

pub struct SWSFlag(u32);
impl SWSFlag {
    pub const FAST_BILINEAR: SWSFlag = SWSFlag(SWS_FAST_BILINEAR);
    pub const BILINEAR: SWSFlag = SWSFlag(SWS_BILINEAR);
    pub const BICUBIC: SWSFlag = SWSFlag(SWS_BICUBIC);
    pub const X: SWSFlag = SWSFlag(SWS_X);
    pub const POINT: SWSFlag = SWSFlag(SWS_POINT);
    pub const AREA: SWSFlag = SWSFlag(SWS_AREA);
    pub const BICUBLIN: SWSFlag = SWSFlag(SWS_BICUBLIN);
    pub const GAUSS: SWSFlag = SWSFlag(SWS_GAUSS);
    pub const SINC: SWSFlag = SWSFlag(SWS_SINC);
    pub const LANCZOS: SWSFlag = SWSFlag(SWS_LANCZOS);
    pub const SPLINE: SWSFlag = SWSFlag(SWS_SPLINE);
}

pub struct ScalingContext(*mut SwsContext);

impl ScalingContext {
    pub fn new(
        source_width: i32,
        source_height: i32,
        source_format: AVPixelFormat,
        dest_width: i32,
        dest_height: i32,
        dest_format: AVPixelFormat,
        flags: SWSFlag)
    -> ScalingContext {
        let sws = unsafe {
            sws_getContext(
                source_width,
                source_height,
                source_format,
                dest_width,
                dest_height,
                dest_format,
                flags.0 as i32,
                null_mut(),
                null_mut(),
                null_mut(),)
        };
        ScalingContext(sws)
    }

    pub fn scale(&self,
        source: &Frame,
        dest: &Frame,
        y_offset: i32,
        height: i32) -> i32 {
        unsafe {
            sws_scale(self.0,
                      ::std::mem::transmute((*source.0).data.as_ptr()),
                      (*source.0).linesize.as_ptr(),
                      y_offset,
                      height,
                      ::std::mem::transmute((*dest.0).data.as_ptr()),
                      (*dest.0).linesize.as_ptr())
        }
    }

    pub fn scale_picture(&self,
        source: &Frame,
        dest: &AVPicture,
        y_offset: i32,
        height: i32) 
    -> i32 {
        unsafe {
            sws_scale(self.0,
                ::std::mem::transmute((*source.0).data.as_ptr()),
                (*source.0).linesize.as_ptr(),
                y_offset,
                height,
                ::std::mem::transmute(dest.data.as_ptr()),
                dest.linesize.as_ptr())
        }
    }
}

impl Drop for ScalingContext {
    fn drop(&mut self) {
        unsafe {
            sws_freeContext(self.0)
        }
    }
}
