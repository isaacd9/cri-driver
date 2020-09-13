use runtime::VersionRequest;
use runtime::runtime_service_client::RuntimeServiceClient;
// Include the `items` module, which is generated from items.proto.
pub mod runtime {
    include!(concat!(env!("OUT_DIR"), "/runtime.v1alpha2.rs"));
}

pub fn create_version_request(version: String) -> runtime::VersionRequest {
    let mut req = VersionRequest::default();
    req.version = version;
    req
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = RuntimeServiceClient::connect("http://localhost:3000").await?;

    let version_req = create_version_request("0.0.1-unnamed-rust-cri-client".to_string());
    let request = tonic::Request::new(version_req);
    let response = client.version(request).await?;

    println!("RESPONSE={:?}", response.into_inner());
    Ok(())
}
