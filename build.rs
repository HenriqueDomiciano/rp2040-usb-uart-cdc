use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    // Put memory.x in output directory
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();

    println!("cargo:rustc-link-search={}", out.display());

    // Re-run only if memory.x changes
    println!("cargo:rustc-link-arg=-Tdefmt.x");   
    println!("cargo:rerun-if-changed=memory.x"); 
}