use std::hash::{DefaultHasher, Hash, Hasher};
use std::{env, fs};

fn main() {
    let mut identity = DefaultHasher::new();
    env::var("TARGET").unwrap().hash(&mut identity);
    env!("CARGO_PKG_VERSION").hash(&mut identity);
    // Cargo.lock identifies Naga and its dependencies. The two source files
    // own WGSL parsing, validation, SPIR-V options and reflection.
    for path in ["../../Cargo.lock", "src/shader.rs", "src/vulkan/pipeline.rs"] {
        println!("cargo:rerun-if-changed={path}");
        fs::read(path).unwrap().hash(&mut identity);
    }
    println!("cargo:rustc-env=BOXBOX_SHADER_COMPILER={:016x}", identity.finish());
}
