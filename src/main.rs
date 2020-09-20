#[macro_use]
extern crate log;
mod config;

use crate::config::read_yaml_config;
use runtime::runtime_service_client::RuntimeServiceClient;
use runtime::runtime_service_server::RuntimeService;
use runtime::*;

pub mod runtime {
    include!(concat!(env!("OUT_DIR"), "/runtime.v1alpha2.rs"));
}

async fn get_and_print_version(
    client: &mut RuntimeServiceClient<tonic::transport::Channel>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Connecting to peer at {}", ADDR);

    let version_req = VersionRequest {
        version: String::from("0.0.1-unnamed-rust-cri-client"),
    };
    let request = tonic::Request::new(version_req);
    let response = client.version(request).await?;

    let msg = response.into_inner();

    info!("Peer is {}@{}", msg.runtime_name, msg.runtime_version);
    Ok(())
}

async fn run_pod_sanbdox() -> Result<(), Box<dyn std::error::Error>> {
    let req = RunPodSandboxRequest {
        config: Some(PodSandboxConfig {
            linux: Some(LinuxPodSandboxConfig {
                cgroup_parent: String::from(""),
                security_context: Some(LinuxSandboxSecurityContext {
                    namespace_options: Some(NamespaceOption {
                        network: NamespaceMode::Node as i32,
                        pid: NamespaceMode::Container as i32,
                        ipc: NamespaceMode::Pod as i32,
                        target_id: String::from(""),
                    }),
                }),
            }),
        }),
        runtime_handler: String::from(""),
    };
    info!("Creating pod sandbox {}");

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
    get_and_print_version(&mut client).await?;

    let stdin = std::io::stdin();
    let config = read_yaml_config(stdin);
    println!("{:?}", config);
    Ok(())
}
