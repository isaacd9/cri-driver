#[macro_use]
extern crate log;

use runtime::runtime_service_client::RuntimeServiceClient;
use runtime::runtime_service_server::RuntimeService;
use runtime::VersionRequest;

pub mod runtime {
    include!(concat!(env!("OUT_DIR"), "/runtime.v1alpha2.rs"));
}

pub fn create_version_request(version: String) -> runtime::VersionRequest {
    let mut req = VersionRequest::default();
    req.version = version;
    req
}

async fn get_version(
    client: &mut RuntimeServiceClient<tonic::transport::Channel>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Connecting to peer at {}", ADDR);

    let version_req = create_version_request("0.0.1-unnamed-rust-cri-client".to_string());
    let request = tonic::Request::new(version_req);
    let response = client.version(request).await?;

    let msg = response.into_inner();

    info!("Peer is {}@{}", msg.runtime_name, msg.runtime_version);
    Ok(())
}

const ADDR: &'static str = "http://localhost:3000";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    let mut client = RuntimeServiceClient::connect(ADDR).await?;
    get_version(&mut client).await?;
    Ok(())
}
