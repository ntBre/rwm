fn main() {
    println!("cargo:rustc-link-arg=-lX11");
    println!("cargo:rustc-link-arg=-lXft");
}
