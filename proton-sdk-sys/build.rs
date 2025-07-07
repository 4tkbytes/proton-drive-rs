use anyhow::*;
use std::result::Result::Ok;
use std::{
    env,
    io::{Read, Result, Write},
    path::PathBuf,
};

fn main() -> Result<()> {
    prost_build::Config::new().compile_protos(
        &["protos/account.proto", "protos/drive.proto"],
        &["protos/"],
    )?;

    println!("cargo:rerun-if-changed=protos/account.proto");
    println!("cargo:rerun-if-changed=protos/drive.proto");

    if let Ok(lib_dir) = env::var("PROTON_SDK_LIB_DIR") {
        let platform = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".to_string());
        let lib_dir_path = PathBuf::from(lib_dir);
        if lib_dir_path.exists() {
            println!(
                "cargo:warning=Using PROTON_SDK_LIB_DIR: {}",
                lib_dir_path.display()
            );
            copy_libs_to_target(&lib_dir_path, &platform);
            println!("cargo:warning=Successfully set up native libraries from PROTON_SDK_LIB_DIR");
            return Ok(());
        } else {
            println!(
                "cargo:warning=PROTON_SDK_LIB_DIR path does not exist: {}",
                lib_dir_path.display()
            );
            panic!("Build failed: PROTON_SDK_LIB_DIR path does not exist");
        }
    }

    // if let Err(e) = download_libs() {
    //     eprintln!("Warning: Unable to download libraries: {:?}", e);
    //     eprintln!("You may need to build the native libraries manually.");
    //     panic!("Build failed...")
    // }

    Ok(())
}

fn download_libs() -> anyhow::Result<()> {
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    let (platform, arch) = parse_target(&target)?;

    // Check if user provided a custom path to native libraries
    if let Ok(custom_path) = env::var("PROTON_SDK_NATIVE_PATH") {
        let custom_lib_dir = PathBuf::from(custom_path);
        if custom_lib_dir.exists() {
            println!(
                "cargo:warning=Using custom native library path: {}",
                custom_lib_dir.display()
            );

            // Copy libraries to target directory for runtime access
            copy_libs_to_target(&custom_lib_dir, &platform)?;

            println!("cargo:warning=Successfully set up native libraries from custom path");
            return Ok(());
        } else {
            println!(
                "cargo:warning=Custom path {} does not exist, falling back to download",
                custom_lib_dir.display()
            );
        }
    }

    // Check if this is an unsupported ARM64 platform
    if arch == "arm64" && (platform == "windows" || platform == "linux") {
        print_arm64_error(&platform);
        return Err(anyhow!(
            "ARM64 builds for {} require manual compilation",
            platform
        ));
    }

    let runtime_id = format!("{}-{}", platform, arch);
    let version = env::var("PROTON_SDK_VERSION").unwrap_or_else(|_| "v1.0.0".to_string());

    println!(
        "cargo:warning=Downloading native libraries for {}",
        runtime_id
    );

    // Construct download URL
    let file_extension = if platform == "win" { "zip" } else { "tar.gz" };
    let filename = format!("proton-sdk-native-{}.{}", runtime_id, file_extension);
    let url = format!(
        "https://github.com/4tkbytes/proton-sdk-rs/releases/download/{}/{}",
        version, filename
    );

    // Create native libs directory in OUT_DIR
    let native_libs_dir = out_dir.join("native-libs");
    std::fs::create_dir_all(&native_libs_dir)?;

    // Download and extract
    download_and_extract(&url, &native_libs_dir, file_extension)?;

    // Copy libraries to target directory for runtime access
    copy_libs_to_target(&native_libs_dir, &platform)?;

    println!(
        "cargo:warning=Successfully set up native libraries for {}",
        runtime_id
    );

    Ok(())
}

fn copy_libs_to_target(source_dir: &PathBuf, platform: &str) -> anyhow::Result<()> {
    // Get the target directory where the executable will be placed
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let target_dir = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            // Default to target directory relative to workspace root
            let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
            PathBuf::from(manifest_dir).parent().unwrap().join("target")
        });

    let exe_dir = target_dir.join(&profile);

    // Ensure the executable directory exists
    std::fs::create_dir_all(&exe_dir)?;

    println!(
        "cargo:warning=Copying native libraries to executable directory: {}",
        exe_dir.display()
    );

    // Copy all library files to the executable directory
    if source_dir.exists() {
        for entry in std::fs::read_dir(source_dir)? {
            let entry = entry?;
            let file_path = entry.path();

            if file_path.is_file() {
                let file_name = file_path
                    .file_name()
                    .ok_or_else(|| anyhow!("Invalid file name"))?;

                // Check if it's a library file we care about
                let extension = file_path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("");

                let is_library = match platform {
                    "windows" => extension == "dll" || extension == "exe",
                    "linux" => extension == "so" || file_name.to_string_lossy().contains(".so."),
                    "osx" => extension == "dylib",
                    _ => true, // Copy everything if unknown platform
                };

                if is_library {
                    let dest_path = exe_dir.join(file_name);
                    std::fs::copy(&file_path, &dest_path)?;
                    println!(
                        "cargo:warning=Copied {} to {}",
                        file_name.to_string_lossy(),
                        dest_path.display()
                    );
                }
            }
        }
    }

    // Set up library search path for the linker (build time)
    println!("cargo:rustc-link-search=native={}", source_dir.display());

    // Add library linking instructions based on platform
    match platform {
        "windows" => {
            println!("cargo:rustc-link-lib=dylib=Proton.Sdk.Drive.CExports");
        }
        "linux" | "osx" => {
            println!("cargo:rustc-link-lib=dylib=Proton.Sdk.Drive.CExports");
        }
        _ => {}
    }

    Ok(())
}

fn download_and_extract(url: &str, dest_dir: &PathBuf, file_extension: &str) -> anyhow::Result<()> {
    use std::process::Command;

    println!("cargo:warning=Downloading from: {}", url);

    // Create a temporary file for the download
    let temp_file = dest_dir
        .parent()
        .unwrap()
        .join(format!("temp_download.{}", file_extension));

    // Use curl to download the file
    let output = Command::new("curl")
        .args(&[
            "-L", // Follow redirects
            "-o", // Output to file
            temp_file.to_str().unwrap(),
            url,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Curl failed: {}", stderr));
    }

    // Check if the file was actually downloaded
    if !temp_file.exists() {
        return Err(anyhow!("Download failed: temporary file not created"));
    }

    println!("cargo:warning=Download completed, extracting...");

    // Read the downloaded file
    let data = std::fs::read(&temp_file)?;

    // Clean up temp file
    std::fs::remove_file(&temp_file).ok(); // Ignore errors on cleanup

    // Extract based on file type
    match file_extension {
        "zip" => extract_zip(&data, dest_dir)?,
        "tar.gz" => extract_tar_gz(&data, dest_dir)?,
        _ => return Err(anyhow!("Unsupported archive format: {}", file_extension)),
    }

    println!(
        "cargo:warning=Successfully extracted native libraries to {}",
        dest_dir.display()
    );

    Ok(())
}

fn process_response(
    response: &[u8],
    response_str: String,
    dest_dir: &PathBuf,
    file_extension: &str,
) -> anyhow::Result<()> {
    // Parse headers to find content length and locate body
    let mut content_length = 0;
    let mut body_start = 0;
    let mut in_headers = true;

    for (i, line) in response_str.lines().enumerate() {
        if in_headers {
            if line.is_empty() {
                // Empty line marks end of headers
                in_headers = false;
                // Calculate byte position of body start
                body_start = response_str
                    .lines()
                    .take(i + 1)
                    .map(|l| l.len() + 2) // +2 for \r\n
                    .sum();
                break;
            } else if line.to_lowercase().starts_with("content-length:") {
                if let Some(len_str) = line.split(':').nth(1) {
                    content_length = len_str.trim().parse().unwrap_or(0);
                }
            }
        }
    }

    if content_length > 0 {
        println!("cargo:warning=Downloading {} bytes...", content_length);
    }

    // Extract body from response
    let body = &response[body_start..];

    if body.is_empty() {
        return Err(anyhow!("Empty response body"));
    }

    // Extract based on file type
    match file_extension {
        "zip" => extract_zip(body, dest_dir)?,
        "tar.gz" => extract_tar_gz(body, dest_dir)?,
        _ => return Err(anyhow!("Unsupported archive format: {}", file_extension)),
    }

    println!(
        "cargo:warning=Successfully extracted native libraries to {}",
        dest_dir.display()
    );

    Ok(())
}

fn extract_zip(data: &[u8], dest_dir: &PathBuf) -> anyhow::Result<()> {
    use std::io::Cursor;

    let reader = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = dest_dir.join(file.name());

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p)?;
                }
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;

            // Set executable permissions on Unix-like systems
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))?;
                }
            }
        }
    }

    Ok(())
}

fn extract_tar_gz(data: &[u8], dest_dir: &PathBuf) -> anyhow::Result<()> {
    use flate2::read::GzDecoder;
    use std::io::Cursor;
    use tar::Archive;

    let reader = Cursor::new(data);
    let gz = GzDecoder::new(reader);
    let mut archive = Archive::new(gz);

    archive.unpack(dest_dir)?;

    Ok(())
}

fn print_arm64_error(platform: &String) {
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let target_dir = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
            PathBuf::from(manifest_dir).parent().unwrap().join("target")
        });
    let exe_dir = target_dir.join(&profile);

    eprintln!(
        "
==============================================================================================
ERROR WHILE RUNNING PROTON-SDK-SYS BUILD
==============================================================================================
This system is running on an unsupported ARM64 platform ({})...

Hi there! If you are reading this message, that means the libraries are not loaded. 
Sorry, I have not been able to get my build script to work under ARM64 (Linux or Windows). 
macOS people should be fine. 

To fix this issue, please clone the repository and run the build script. The build script
should guide you through creating the library. 

Run the following commands:

```bash
git clone https://github.com/4tkbytes/proton-sdk-rs
cd proton-sdk-rs
python build.py --exclude rust
```

After build, you'll find the library at: native-libs/{}-arm64/
   
Next, set environment variable and rebuild:
```bash
export PROTON_SDK_LIB_DIR=/path/to/your/native-libs/{}-arm64
cargo build
```

The build script will automatically copy the libraries to your executable directory.

You will require the following dependencies: 
    - git
    - dotnet (.NET 9.0 or later)
    - cargo
    - rustc
    - go (1.21 or later)
    - gcc (not clang)

Of course, the script will guide you through the process of installing them. 

If you wish to report any additional issues, create an issue at 
https://github.com/4tkbytes/proton-sdk-rs/issues and add the log
of the build script. 

Again, sorry for the inconvenience ヽ(*。>Д<)o゜
==============================================================================================
    ",
        platform, platform, platform
    );
}

fn parse_target(target: &str) -> anyhow::Result<(String, String)> {
    let parts: Vec<&str> = target.split('-').collect();

    let (arch, platform) = match parts.as_slice() {
        ["x86_64", "pc", "windows", ..] => ("x64", "win"),
        ["aarch64", "pc", "windows", ..] => ("arm64", "win"),
        ["i686", "pc", "windows", ..] => {
            panic!("Please use a 64 bit operating system or build the library yourself...")
        }

        ["x86_64", "unknown", "linux", ..] => ("x64", "linux"),
        ["aarch64", "unknown", "linux", ..] => ("arm64", "linux"),
        ["i686", "unknown", "linux", ..] => {
            panic!("Please use a 64 bit operating system or build the library yourself...")
        }

        ["x86_64", "apple", "darwin"] => ("x64", "osx"),
        ["aarch64", "apple", "darwin"] => ("arm64", "osx"),

        _ => {
            return Err(anyhow!("Unsupported target: {}", target));
        }
    };

    Ok((platform.to_string(), arch.to_string()))
}
