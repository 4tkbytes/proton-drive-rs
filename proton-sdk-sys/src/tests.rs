use super::*;

use crate::ProtonSDKLib;
use std::ffi::c_void;

/// Test function that creates and destroys a cancellation token source
pub fn test_cancellation_token() -> anyhow::Result<()> {
    unsafe {
        let sdk = ProtonSDKLib::instance()?;
        
        // Get the function from the library
        let create_fn: libloading::Symbol<unsafe extern "C" fn() -> isize> = 
            sdk.sdk_library.get(b"cancellation_token_source_create")?;
        
        let free_fn: libloading::Symbol<unsafe extern "C" fn(isize)> = 
            sdk.sdk_library.get(b"cancellation_token_source_free")?;
        
        // Call the function
        let token_handle = create_fn();
        
        println!("Created cancellation token with handle: {}", token_handle);
        
        // Verify we got a valid handle (should be non-zero)
        if token_handle != 0 {
            println!("✓ Cancellation token created successfully");
            
            // Clean up
            free_fn(token_handle);
            println!("✓ Cancellation token freed successfully");
            
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to create cancellation token - received null handle"))
        }
    }
}

#[test]
fn test_cancellation_token_creation() {
    match test_cancellation_token() {
        Ok(()) => println!("Test passed!"),
        Err(e) => panic!("Test failed: {}", e),
    }
}

#[test]
fn test_protobuf_generation() {
    // This test will fail to compile if protobuf generation didn't work
    // Try to create a message from your proto files
    
    // Example for account.proto messages
    let _options = ProtonClientOptions {
        app_version: "test".to_string(),
        user_agent: None,
        base_url: None,
        disable_tls_pinning: None,
        ignore_ssl_certificate_errors: None,
        logger_provider_handle: None,
    };
    
    println!("Protobuf generation working!");
}