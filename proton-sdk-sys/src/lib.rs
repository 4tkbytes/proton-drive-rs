#[cfg(test)]
pub mod tests;
pub mod client;

pub struct ProtonSDKLib {
    pub sdk_library: Library,
    pub location: PathBuf,
}

impl ProtonSDKLib {
    /// This function loads the library and returns an instance
    /// of the ProtonSDKLib
    pub unsafe fn load() -> anyhow::Result<Self> {
        let (lib, location) = Self::call_sdk_lib()?;
        Ok(Self {
            sdk_library: lib,
            location,
        })
    }

    unsafe fn call_sdk_lib() -> Result<(Library, PathBuf), libloading::Error> {
        let library_path = Self::get_library_path();
        
        match Library::new(&library_path) {
            Ok(lib) => Ok((lib, library_path)),
            Err(e) => {
                eprintln!("Failed to load library from {}: {}", library_path.display(), e);
                
                // Try fallback locations
                for fallback_path in Self::get_fallback_paths() {
                    if let Ok(lib) = Library::new(&fallback_path) {
                        return Ok((lib, library_path));
                    }
                }
                
                Err(e)
            }
        }
    }

    fn get_library_path() -> PathBuf {
        let (runtime_id, lib_name) = Self::get_platform_info();
        PathBuf::from(format!("native-libs/{}/publish/{}", runtime_id, lib_name))
    }

    fn get_platform_info() -> (&'static str, &'static str) {
        #[cfg(target_os = "windows")]
        {
            let runtime_id = match std::env::consts::ARCH {
                "x86_64" => "win-x64",
                "x86" => "win-x86",
                "aarch64" => "win-arm64",
                _ => panic!("Unsupported Windows architecture: {}", std::env::consts::ARCH),
            };
            (runtime_id, "proton_drive_sdk.dll")
        }
        
        #[cfg(target_os = "linux")]
        {
            let runtime_id = match std::env::consts::ARCH {
                "x86_64" => "linux-x64",
                "x86" => "linux-x86",
                "aarch64" => "linux-arm64",
                "arm" => "linux-arm",
                _ => panic!("Unsupported Linux architecture: {}", std::env::consts::ARCH),
            };
            (runtime_id, "libproton_drive_sdk.so")
        }
        
        #[cfg(target_os = "macos")]
        {
            let runtime_id = match std::env::consts::ARCH {
                "x86_64" => "osx-x64",
                "aarch64" => "osx-arm64",
                _ => panic!("Unsupported macOS architecture: {}", std::env::consts::ARCH),
            };
            (runtime_id, "libproton_drive_sdk.dylib")
        }
        
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            panic!("Unsupported operating system: {}", std::env::consts::OS);
        }
    }

    fn get_fallback_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let (runtime_id, lib_name) = Self::get_platform_info();
        
        paths.push(PathBuf::from(format!("native-libs/{}/{}", runtime_id, lib_name)));
        
        paths.push(PathBuf::from(lib_name));
        
        paths.push(PathBuf::from(format!("libs/{}", lib_name)));
        
        #[cfg(target_os = "linux")]
        {
            paths.push(PathBuf::from(format!("/usr/local/lib/{}", lib_name)));
            paths.push(PathBuf::from(format!("/usr/lib/{}", lib_name)));
        }
        
        #[cfg(target_os = "macos")]
        {
            paths.push(PathBuf::from(format!("/usr/local/lib/{}", lib_name)));
            paths.push(PathBuf::from(format!("/opt/homebrew/lib/{}", lib_name)));
        }
        
        paths
    }
}

use libloading::Library;
use std::path::PathBuf;

#[repr(C)]
struct ByteArray {
    pointer: *const u8,
    length: usize,
}

