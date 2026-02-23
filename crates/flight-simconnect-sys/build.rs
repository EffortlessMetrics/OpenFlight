use std::env;
#[cfg(feature = "static")]
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(simconnect_static_available)");

    // Only build on Windows
    if env::var("CARGO_CFG_TARGET_OS").unwrap() != "windows" {
        return;
    }

    // For static linking, we need to link against SimConnect.lib
    #[cfg(feature = "static")]
    {
        // Try to find SimConnect SDK
        let mut sdk_paths = vec![
            String::from(r"C:\MSFS SDK\SimConnect SDK\lib"),
            String::from(
                r"C:\Program Files (x86)\Microsoft Games\Microsoft Flight Simulator X SDK\SDK\Core Utilities Kit\SimConnect SDK\lib",
            ),
        ];
        sdk_paths.push(env::var("SIMCONNECT_SDK_PATH").unwrap_or_default());

        let mut found_lib = false;
        for path in &sdk_paths {
            if !path.is_empty() {
                let lib_path = PathBuf::from(path);
                if lib_path.join("SimConnect.lib").exists() {
                    println!("cargo:rustc-link-search=native={}", path);
                    println!("cargo:rustc-link-lib=static=SimConnect");
                    println!("cargo:rustc-cfg=simconnect_static_available");
                    found_lib = true;
                    break;
                }
            }
        }

        if !found_lib {
            println!(
                "cargo:warning=SimConnect.lib not found. Set SIMCONNECT_SDK_PATH environment variable or install MSFS SDK."
            );
            println!("cargo:warning=Falling back to dynamic linking.");
        }
    }

    // For dynamic linking, we don't need to link at build time
    #[cfg(feature = "dynamic")]
    {
        println!("cargo:rustc-cfg=dynamic_simconnect");
    }
}
