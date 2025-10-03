fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use prost-build and tonic-build 0.14.x API
    let mut prost_config = prost_build::Config::new();
    prost_config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    
    // Compile the proto files with prost-build (tonic-build 0.14 integration is complex)
    prost_config.compile_protos(&["proto/flight.v1.proto"], &["proto/"])?;
    
    // Generate version info for breaking change detection
    println!("cargo:rerun-if-changed=proto/flight.v1.proto");
    
    Ok(())
}
