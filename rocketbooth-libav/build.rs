use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for lib in &["avutil", "avformat", "avcodec", "avdevice", "swscale"] {
        println!("cargo:rustc-link-lib={lib}");
    }

    let bindings = bindgen::builder()
        .header("src/bindings.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()?;
    let out_path = PathBuf::from(env::var("OUT_DIR")?);
    bindings.write_to_file(out_path.join("bindings.rs"))?;
    Ok(())
}
