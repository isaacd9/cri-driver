extern crate tonic_build;
use std::env;

fn main() {
    tonic_build::compile_protos("proto/api.proto").unwrap();

    tonic_build::configure()
        .type_attribute(".", "#[derive(Deserialize)]")
        .type_attribute(".", "#[serde(rename_all = \"snake_case\")]")
        .compile(&["proto/service.proto"], &["proto"])
        .unwrap();
}
