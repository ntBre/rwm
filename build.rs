use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=dwm/config.h");
    println!("cargo:rerun-if-changed=dwm/dwm.h");
    println!("cargo:rerun-if-changed=wrapper.h");

    println!("cargo:rustc-link-arg=-Ldwm");
    println!("cargo:rustc-link-arg=-ldwm");
    println!("cargo:rustc-link-arg=-lfontconfig");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/home/brent/packages/rwm/dwm");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg("-I/usr/include/freetype2")
        .clang_arg("-I/usr/include/X11/extensions")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .blocklist_var("numlockmask")
        .blocklist_var("running")
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
