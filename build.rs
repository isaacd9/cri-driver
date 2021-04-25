extern crate prost_build;
extern crate tonic_build;

fn main() {
    tonic_build::compile_protos("proto/api.proto").unwrap();

    let mut config = prost_build::Config::new();

    config
        .type_attribute(".", "#[derive(::serde::Deserialize)]")
        .type_attribute(".", "#[serde(rename_all = \"snake_case\")]")
        .field_attribute(".", "#[serde(default)]");

    tonic_build::configure()
        .compile_with_config(config, &["proto/service.proto"], &["proto"])
        .unwrap();
}
