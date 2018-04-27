extern crate bindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    // Add extra search paths for libraries
    println!("cargo:rustc-link-search=/opt/vc/lib");

    // Instruct cargo to link native libraries
    let link_libs = [
        // ffmpeg
        "avutil",
        "avformat",
        "avcodec",
        "avdevice",
        "swscale",
        // openvg
        "brcmGLESv2",
        "brcmEGL",
        "openmaxil",
        "bcm_host",
        "vcos",
        "vchiq_arm",
        "static=GLESv2_static",
        "static=EGL_static",
        "static=khrn_static",
    ];
    for lib in &link_libs {
        println!("cargo:rustc-link-lib={}", lib);
    }

    let headers = [ "ffmpeg", "openvg", "evdev" ];
    for header in &headers {
        println!("cargo:rerun-if-changed={}-headers.h", header);
    }

    for header in &headers {
        let bindings = bindgen::Builder::default()
            .header(format!("{}-headers.h", header))
            .clang_arg("-I/opt/vc/include")
            .blacklist_type("FP_NAN")
            .blacklist_type("FP_INFINITE")
            .blacklist_type("FP_ZERO")
            .blacklist_type("FP_NORMAL")
            .blacklist_type("FP_SUBNORMAL")
            .generate()
            .expect("Unable to generate bindings");

        let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
        bindings.write_to_file(out_path.join(format!("{}-bindings.rs", header)))
            .expect("Couldn't write bindings!");
    }
}
