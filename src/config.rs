use serde_yaml::{Result, Value};

use std::io::Read;
use types::*;

pub mod types {
    use serde;
    include!(concat!(env!("OUT_DIR"), "/types.rs"));
}

use types::Pod;

pub fn read_yaml_config<R>(reader: R) -> Result<Pod>
where
    R: Read,
{
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
