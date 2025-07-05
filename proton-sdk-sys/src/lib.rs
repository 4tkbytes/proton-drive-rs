use std::path::{Path, PathBuf};
use std::fs;
use std::env;

#[allow(dead_code)]
fn get_runtime_id() -> String {
    let os = if cfg!(target_os = "windows") {
        "win"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "unknown"
    };
    
    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "unknown"
    };
    
    format!("{}-{}", os, arch)
}

#[allow(dead_code)]
fn find_workspace_root() -> Option<PathBuf> {
    let mut current = env::current_dir().ok()?;
    
    loop {
        if current.join("Cargo.toml").exists() && 
           current.join("proton-sdk-sys").exists() {
            return Some(current);
        }
        
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            break;
        }
    }
    
    None
}

#[allow(dead_code)]
fn call_dyn() -> Result<(), Box<dyn std::error::Error>> {
    let runtime_id = get_runtime_id();
    
    let workspace_root = find_workspace_root()
        .ok_or("Could not find workspace root")?;
    
    let native_libs_dir = workspace_root
        .join("proton-sdk-sys")
        .join("native-libs")
        .join(&runtime_id);
    
    if !native_libs_dir.exists() {
        return Err(format!("Native libraries directory not found: {:?}", native_libs_dir).into());
    }
    
    let library_names = if cfg!(target_os = "windows") {
        vec![
            "Proton.Sdk.dll",
            "Proton.Sdk.Drive.dll", 
            "proton_sdk.dll",
            "libproton_drive_sdk.dll",
        ]
    } else if cfg!(target_os = "macos") {
        vec![
            "libProton.Sdk.dylib",
            "libProton.Sdk.Drive.dylib",
            "libproton_sdk.dylib",
            "libproton_drive_sdk.dylib",
        ]
    } else {
        vec![
            "libProton.Sdk.so",
            "libProton.Sdk.Drive.so", 
            "libproton_sdk.so",
            "libproton_drive_sdk.so",
        ]
    };
    
    let mut attempted_paths = Vec::new();
    let mut load_errors = Vec::new();
    
    fn find_libraries_recursive(dir: &Path, names: &[&str]) -> Vec<PathBuf> {
        let mut found = Vec::new();
        
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    found.extend(find_libraries_recursive(&path, names));
                } else if let Some(file_name) = path.file_name() {
                    let file_str = file_name.to_string_lossy().to_lowercase();
                    for &name in names {
                        if file_str == name.to_lowercase() {
                            found.push(path.clone());
                        }
                    }
                }
            }
        }
        
        found
    }
    
    let found_libraries = find_libraries_recursive(&native_libs_dir, &library_names);
    
    if found_libraries.is_empty() {
        return Err(format!(
            "No suitable libraries found in {:?}. Looking for: {:?}", 
            native_libs_dir, 
            library_names
        ).into());
    }
    
    println!("Found {} potential libraries:", found_libraries.len());
    for lib_path in &found_libraries {
        println!("  - {:?}", lib_path);
    }
    
    // attepmting to load each library
    for lib_path in &found_libraries {
        attempted_paths.push(lib_path.clone());
        
        unsafe {
            match libloading::Library::new(lib_path) {
                Ok(_lib) => {
                    println!("✓ Successfully loaded library: {:?}", lib_path);
                    return Ok(());
                }
                Err(e) => {
                    let error_msg = format!("Failed to load {:?}: {}", lib_path, e);
                    println!("✗ {}", error_msg);
                    load_errors.push(error_msg);
                }
            }
        }
    }
    
    Err(format!(
        "Failed to load any library. Attempted paths: {:?}\nErrors: {:?}", 
        attempted_paths, 
        load_errors
    ).into())
}