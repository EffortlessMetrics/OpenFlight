fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use prost-build to generate protobuf types only
    // We'll manually implement the gRPC service traits for now
    prost_build::compile_protos(&["proto/flight.v1.proto"], &["proto"])?;
    
    // Generate version info for breaking change detection
    println!("cargo:rerun-if-changed=proto/flight.v1.proto");
    
    Ok(())
}
