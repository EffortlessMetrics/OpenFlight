fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Prefer an explicit PROTOC env-var; fall back to the vendored binary so
    // the build works on fresh machines without a system `protoc` install.
    if std::env::var_os("PROTOC").is_none()
        && let Ok(path) = protoc_bin_vendored::protoc_bin_path()
    {
        // SAFETY: build scripts run single-threaded.
        unsafe { std::env::set_var("PROTOC", path) };
    }

    // Use prost-build to generate protobuf types only
    // We'll manually implement the gRPC service traits for now
    prost_build::compile_protos(&["proto/flight.v1.proto"], &["proto"])?;

    // Generate version info for breaking change detection
    println!("cargo:rerun-if-changed=proto/flight.v1.proto");

    Ok(())
}
