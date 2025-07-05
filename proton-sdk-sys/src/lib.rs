use std::path::PathBuf;

fn call_dyn() {
    unsafe {
        let sdk_lib = libloading::Library::new("C:/Users/thrib/monolith/proton-sdk-rs/native-libs/Proton.Sdk.dll").unwrap();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_dylib_loading() {
        call_dyn();
    }
}