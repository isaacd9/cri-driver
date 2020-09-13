extern crate tonic_build;
use std::env;

fn main() {
    tonic_build::compile_protos("proto/api.proto").unwrap();
}
