pub mod cancellation;
pub mod data;
pub mod downloads;
pub mod drive;
pub mod logger;
pub mod nodes;
pub mod observability;
pub mod protobufs;
pub mod sessions;
pub mod uploader;

use libloading::Library;
use log::{debug, error, warn};
use std::{
    path::PathBuf,
    sync::{Mutex, Once},
};

pub struct ProtonSDKLib {
    pub sdk_library: Library,
    pub location: PathBuf,
}

static INIT: Once = Once::new();
static mut PROTON_SDK_INSTANCE: Option<ProtonSDKLib> = None;

impl ProtonSDKLib {
    pub fn instance() -> anyhow::Result<&'static Self> {
        unsafe {
            INIT.call_once(|| match Self::load_internal() {
                Ok(instance) => {
                    PROTON_SDK_INSTANCE = Some(instance);
                }
                Err(e) => {
                    error!("Failed to initialise ProtonSDKLib: {}", e);
                }
            });

            // dude stfu i do not care about this error
            #[warn(static_mut_refs)]
            PROTON_SDK_INSTANCE
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Failed to initialise ProtonSDKLib"))
        }
    }

    /// This function loads the library and returns an instance
    /// of the ProtonSDKLib
    unsafe fn load_internal() -> anyhow::Result<Self> {
        let (lib, location) = Self::call_sdk_lib()?;
        Ok(Self {
            sdk_library: lib,
            location,
        })
    }

    unsafe fn call_sdk_lib() -> Result<(Library, PathBuf), libloading::Error> {
        let (_runtime_id, lib_name) = Self::get_platform_info();
        let library_path = PathBuf::from(lib_name);

        match Library::new(&library_path) {
            Ok(lib) => {
                debug!("Loaded SDK library from: {}", library_path.display());
                Ok((lib, library_path))
            }
            Err(e) => {
                warn!(
                    "Failed to load library from {}: {}",
                    library_path.display(),
                    e
                );

                // Try fallback paths
                for fallback_path in Self::get_fallback_paths() {
                    match Library::new(&fallback_path) {
                        Ok(lib) => {
                            debug!(
                                "Loaded SDK library from fallback: {}",
                                fallback_path.display()
                            );
                            return Ok((lib, fallback_path));
                        }
                        Err(fallback_err) => {
                            warn!(
                                "Fallback failed for {}: {}",
                                fallback_path.display(),
                                fallback_err
                            );
                        }
                    }
                }

                Err(e)
            }
        }
    }

    fn get_platform_info() -> (&'static str, &'static str) {
        #[cfg(target_os = "windows")]
        {
            let runtime_id = match std::env::consts::ARCH {
                "x86_64" => "win-x64",
                "x86" => "win-x86",
                "aarch64" => "win-arm64",
                _ => panic!(
                    "Unsupported Windows architecture: {}",
                    std::env::consts::ARCH
                ),
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
        let (_runtime_id, lib_name) = Self::get_platform_info();

        paths.push(PathBuf::from(format!("./{}", lib_name)));
        paths.push(PathBuf::from(format!("./libs/{}", lib_name)));
        paths.push(PathBuf::from(format!("../libs/{}", lib_name)));

        paths.push(PathBuf::from(format!("target/debug/{}", lib_name)));
        paths.push(PathBuf::from(format!("target/release/{}", lib_name)));
        paths.push(PathBuf::from(format!("../target/debug/{}", lib_name)));
        paths.push(PathBuf::from(format!("../target/release/{}", lib_name)));

        paths
    }
}
