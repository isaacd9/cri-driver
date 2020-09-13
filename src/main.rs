// Include the `items` module, which is generated from items.proto.
pub mod runtime {
    include!(concat!(env!("OUT_DIR"), "/runtime.v1alpha2.rs"));
}

fn main() {
    println!("Hello, world!");
}
