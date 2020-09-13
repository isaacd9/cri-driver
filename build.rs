extern crate prost_build;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    prost_build::compile_protos(&["proto/api.proto"],
                                &["proto"]).unwrap();
}
