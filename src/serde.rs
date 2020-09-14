use serde::Deserializer;

pub mod runtime {
    include!(concat!(env!("OUT_DIR"), "/types.rs"));
}

use types::*;
