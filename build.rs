#[cfg(target_os = "freebsd")]
use bindgen;
#[cfg(target_os = "freebsd")]
use std::env;
#[cfg(target_os = "freebsd")]
use std::path::PathBuf;

#[cfg(target_os = "freebsd")]
fn freebsd_bindgen() {
    println!("cargo:rustc-link-lib=procstat");
    println!("cargo:rerun-if-changed=freebsd_wrapper.h");
    let bindings = bindgen::Builder::default()
        .header("freebsd_wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Unable to generate bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("freebsd_bindings.rs"))
        .expect("Couldn't write bindings!");
}

fn main() {
    let is_apple = std::env::var("TARGET")
        .map(|t| t.contains("-apple"))
        .unwrap_or(false);
    let is_ios = std::env::var("CARGO_CFG_TARGET_OS")
        .map(|s| s == "ios")
        .unwrap_or(false);

    if is_apple {
        if !is_ios {
            // DiskArbitration is not available on iOS: https://developer.apple.com/documentation/diskarbitration
            println!("cargo:rustc-link-lib=framework=DiskArbitration");
            // IOKit is not available on iOS: https://developer.apple.com/documentation/iokit
            println!("cargo:rustc-link-lib=framework=IOKit");
        }

        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    }

    #[cfg(target_os = "freebsd")]
    freebsd_bindgen();
}
