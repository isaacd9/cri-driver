use serde_yaml::Result;

use std::io::Read;

pub mod types {
    include!(concat!(env!("OUT_DIR"), "/types.rs"));
}

use types::Pod;

pub fn read_yaml_config<R: Read>(reader: R) -> Result<Pod> {
    serde_yaml::from_reader::<R, Pod>(reader)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_read_yaml_from_config() {
        let test_config = "
        containers:
            - name: \"hello world\"
        ";
        let result = read_yaml_config(test_config.as_bytes()).unwrap();
        println!("{:?}", result)
    }
}
