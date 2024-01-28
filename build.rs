use std::{env, path::PathBuf};

fn main() {
    // println!("cargo:rerun-if-changed=wrapper.h");
    // println!("cargo:rerun-if-changed=/home/brent/packages/dwm/dwm.h");
    // println!("cargo:rerun-if-changed=/home/brent/packages/dwm/dwm.c");
    // println!("cargo:rerun-if-changed=/home/brent/packages/dwm/libdwm.so");

    // println!("cargo:rustc-link-lib=X11");
    // println!("cargo:rustc-link-lib=Xinerama");
    // println!("cargo:rustc-link-lib=fontconfig");
    // println!("cargo:rustc-link-lib=Xft");
    // println!("cargo:rustc-link-lib=freetype");

    println!("cargo:rustc-link-arg=-L../dwm");
    println!("cargo:rustc-link-arg=-ldwm");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg("-I/usr/include/freetype2")
        .clang_arg("-I/usr/include/X11/extensions")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
