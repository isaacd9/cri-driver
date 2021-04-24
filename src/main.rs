#[macro_use]
extern crate log;
mod config;

use crate::config::read_yaml_config;
use rand::Rng;
use runtime::runtime_service_client::RuntimeServiceClient;
use runtime::runtime_service_server::RuntimeService;
use runtime::*;
use std::collections::HashMap;

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

async fn run_pod_sandbox(
    pod_name: &String,
    client: &mut RuntimeServiceClient<tonic::transport::Channel>,
) -> Result<(std::String), Box<dyn std::error::Error>> {
    let mut security_context = LinuxSandboxSecurityContext::default();

    security_context.namespace_options = Some(NamespaceOption {
        network: NamespaceMode::Node as i32,
        pid: NamespaceMode::Container as i32,
        ipc: NamespaceMode::Pod as i32,
        target_id: String::from(""),
    });

    let mut pod_sandbox_config = PodSandboxConfig::default();
    pod_sandbox_config.metadata = Some(PodSandboxMetadata {
        name: pod_name.clone(),
        uid: String::from("my user"),
        namespace: String::from("my-test-name-namespace"),
        attempt: 0,
    });
    pod_sandbox_config.linux = Some(LinuxPodSandboxConfig {
        cgroup_parent: String::from(""),
        security_context: Some(security_context),
        sysctls: HashMap::default(),
    });

    let request = RunPodSandboxRequest {
        config: Some(pod_sandbox_config),
        runtime_handler: String::from(""),
    };

    info!("Creating pod sandbox {:?}", request);
    let response = client.run_pod_sandbox(request).await?;
    let msg = response.into_inner();
    info!("peer responded {:?}", msg);
    Ok(())
}

async fn create_containers(
    pod_sandbox_id: &String,
    client: &mut RuntimeServiceClient<tonic::transport::Channel>,
) -> Result<(), Box<dyn std::error::Error>> {
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

    let mut rng = rand::thread_rng();
    let name = format!("my-test-pod-{}", rng.gen::<u128>());
    let stdin = std::io::stdin();
    //let config = read_yaml_config(stdin);
    //println!("{:?}", config);
    run_pod_sandbox(&name, &mut client).await?;
    Ok(())
}
