use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for lib in &["avutil", "avformat", "avcodec", "avdevice", "swscale"] {
        println!("cargo:rustc-link-lib={lib}");
    }

    let bindings = bindgen::builder()
        .header("src/bindings.h")
        .blocklist_var("FP_ZERO")
        .blocklist_var("FP_INFINITE")
        .blocklist_var("FP_NAN")
        .blocklist_var("FP_NORMAL")
        .blocklist_var("FP_SUBNORMAL")
        .newtype_enum("AVMediaType")
        .newtype_enum("AVPixelFormat")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()?;
    let out_path = PathBuf::from(env::var("OUT_DIR")?);
    bindings.write_to_file(out_path.join("bindings.rs"))?;
    Ok(())
}
