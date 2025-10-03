fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use prost-build to generate protobuf types
    // We'll manually implement gRPC service traits
    prost_build::compile_protos(&["proto/flight.v1.proto"], &["proto"])?;
    
    // Generate version info for breaking change detection
    println!("cargo:rerun-if-changed=proto/flight.v1.proto");
    
    Ok(())
}
