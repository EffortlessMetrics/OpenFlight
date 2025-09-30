fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure tonic build with feature gates
    let config = tonic_build::configure();
    
    // Always build both for now during development
    let config = config
        .build_server(true)
        .build_client(true)
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    
    config.compile(&["proto/flight.v1.proto"], &["proto/"])?;
    
    // Generate version info for breaking change detection
    println!("cargo:rerun-if-changed=proto/flight.v1.proto");
    
    Ok(())
}
