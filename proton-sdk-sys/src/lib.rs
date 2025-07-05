use std::path::PathBuf;

fn call_dyn() {
    let lib_path = get_library_path();
    println!("{}", &lib_path.to_str().unwrap());
    unsafe {
        let sdk_lib = libloading::Library::new(&lib_path.to_str().unwrap()).unwrap();
    }
}

fn get_library_path() -> PathBuf{
    let base_dir = PathBuf::from("../native-libs");
    
    let possible_libraries = if cfg!(target_os = "windows") {
        vec![
            "proton_sdk.dll",
            "libproton_sdk.dll",
            "proton_drive_sdk.dll",
            "libproton_drive_sdk.dll",
        ]
    } else if cfg!(target_os = "linux") {
        vec![
            "libproton_sdk.dll",
            "libproton_sdk.so",
            "proton_sdk.dll",
            "proton_sdk.so",
        ]
    } else if cfg!(target_os = "macos") {
        vec![
            "libproton_sdk.dylib",
            "libproton_sdk.dll",
            "proton_sdk.dylib",
            "proton_sdk.dll",
        ]
    } else {
        vec!["libproton_sdk.dll", "proton_sdk.dll"]
    };
    
    // Try each possible library name until we find one that exists
    for lib_name in possible_libraries.clone() {
        let full_path = base_dir.join(lib_name);
        if full_path.exists() {
            println!("Found library: {}", full_path.display());
            return full_path;
        }
    }
    
    // Fallback to the first option if none exist
    let fallback = base_dir.join(possible_libraries[0]);
    println!("No library found, using fallback: {}", fallback.display());
    fallback
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_dylib_loading() {
        call_dyn();
    }
}