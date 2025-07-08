use anyhow::*;
use std::{
    env,
    fs,
    io::Result,
    path::{Path, PathBuf},
};

fn main() -> anyhow::Result<()> {
    println!("cargo:warning=PROTON_SDK_LIB_DIR={:?}", std::env::var("PROTON_SDK_LIB_DIR"));
    prost_build::Config::new().compile_protos(
        &["protos/account.proto", "protos/drive.proto"],
        &["protos/"],
    )?;

    copy_dlls_to_exe_dir()?;

    println!("cargo:rerun-if-changed=protos/account.proto");
    println!("cargo:rerun-if-changed=protos/drive.proto");
    Ok(())
}

fn get_platform_lib_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "proton_drive_sdk.dll"
    }
    #[cfg(target_os = "linux")]
    {
        "libproton_drive_sdk.so"
    }
    #[cfg(target_os = "macos")]
    {
        "libproton_drive_sdk.dylib"
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        compile_error!("Unsupported operating system");
    }
}

fn copy_dlls_to_exe_dir() -> anyhow::Result<()> {
    let lib_dir = env::var("PROTON_SDK_LIB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| panic!("Error: PROTON_SDK_LIB_DIR environment variable must be set to the directory containing the SDK libraries."));

    if !lib_dir.is_dir() {
        panic!("PROTON_SDK_LIB_DIR does not point to a valid directory: {}", lib_dir.display());
    }

    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let target_dir = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
            Path::new(&manifest_dir).parent().unwrap().join("target")
        });
    let exe_dir = target_dir.join(&profile);

    fs::create_dir_all(&exe_dir)?;

    #[cfg(target_os = "windows")]
    let exts = ["dll"];
    #[cfg(target_os = "linux")]
    let exts = ["so"];
    #[cfg(target_os = "macos")]
    let exts = ["dylib"];

    let mut found = false;
    for entry in fs::read_dir(&lib_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if exts.iter().any(|&wanted| ext.eq_ignore_ascii_case(wanted)) {
                    let dest = exe_dir.join(path.file_name().unwrap());
                    fs::copy(&path, &dest)?;
                    println!("Copied {} to {}", path.display(), dest.display());

                    println!("cargo:rerun-if-changed={}", path.display());
                    found = true;
                }
            }
        }
    }

    if !found {
        panic!(
            "No library files with extensions {:?} found in PROTON_SDK_LIB_DIR: {}",
            exts,
            lib_dir.display()
        );
    }

    Ok(())
}