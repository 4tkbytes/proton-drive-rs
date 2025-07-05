fn main() {
    // Print the current directory for debugging
    println!("cargo:warning=Current dir: {:?}", std::env::current_dir().unwrap());
    
    // Check if native-libs directory exists
    let native_libs = std::path::Path::new("../native-libs");
    if !native_libs.exists() {
        println!("cargo:warning=native-libs directory does not exist!");
        return;
    }
    
    // List what's in the directory
    if let Ok(entries) = std::fs::read_dir("../native-libs") {
        for entry in entries {
            if let Ok(entry) = entry {
                println!("cargo:warning=Found library: {:?}", entry.file_name());
            }
        }
    }
    
    // Don't do static linking on Windows - we'll load dynamically
    // println!("cargo:rustc-link-search=native=../native-libs");
    // println!("cargo:rustc-link-lib=dylib=proton_sdk");
    
    println!("cargo:rerun-if-changed=../native-libs");
    println!("cargo:rerun-if-changed=build.rs");
}