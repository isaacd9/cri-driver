// Include the `items` module, which is generated from items.proto.
pub mod runtime {
    include!(concat!(env!("OUT_DIR"), "/runtime.v1alpha2.rs"));
}

pub fn create_version_request(version: String) -> runtime::VersionRequest {
    let mut req = runtime::VersionRequest::default();
    req.version = version;
    req
}

fn main() {
    let v = create_version_request("0.0.1-unnamed-rust-cri-client".to_string());
    println!("{:?}", v);
}
