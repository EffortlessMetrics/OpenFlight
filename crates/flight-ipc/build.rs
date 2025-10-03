use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use prost-build for basic protobuf generation
    let proto_file = "proto/flight.v1.proto";
    let proto_dir = "proto";
    
    // Generate basic protobuf types
    prost_build::compile_protos(&[proto_file], &[proto_dir])?;
    
    // Generate version info for breaking change detection
    println!("cargo:rerun-if-changed={}", proto_file);
    
    Ok(())
}
