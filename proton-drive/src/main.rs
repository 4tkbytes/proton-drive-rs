use std::{env, io::{self, Write}};
use proton_sdk_rs::sessions::{SessionBuilder, SessionPlatform};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    if let Err(_) = dotenv::dotenv() {
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let env_path = workspace_root.join(".env");
        dotenv::from_path(env_path).ok();
    }

    println!("Proton Drive Testing App, do not use with real credentials YET");
    let username = env::var("PROTON_USERNAME").expect("You must provide a username in the .env file");
    let password = env::var("PROTON_PASSWORD").expect("You must provide a password in the .env file");

    println!("Creating session for user: {}", username);
    println!("Using credentials: username={}, password={}chars", username, password.len());

    let session_result = SessionBuilder::new(username, password)
        // .with_app_version(SessionPlatform::Windows, "proton-drive-rs", "1.0.0")
        
        // Hi there whomever is looking at this: I am having this issue, where I cannot access my custom made app. 
        // I am hit with the error: 
        //      Error details: code=5003, message=OutdatedApp: This version of the app is no longer supported, please update to continue using the app
        // To test out the reason, I have created a function that spoofs it to rclone. That works. 
        // It seems that your backend does not have any implementation of custom apps such as that in the function below:
        //      .with_app_version(SessionPlatform::Windows, "proton-drive-rs", "1.0.0")
        // Please fix the issue when you can. Thanks :3
        .with_rclone_app_version_spoof()
        .with_request_response_callback(|data| {
            let data_str = String::from_utf8_lossy(data);
            println!("HTTP: {} bytes", data.len());
            if data.len() < 500 {
                println!("   Content: {}", data_str);
            } else {
                println!("   Content (truncated): {}...", &data_str[..200]);
            }
        })
        .with_secret_requested_callback(|| {
            print!("2FA/Secret required. Enter 'y' to continue: ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let result = input.trim().to_lowercase() == "y";
            println!("   Secret callback returning: {}", result);
            result
        })
        .with_tokens_refreshed_callback(|tokens| {
            println!("Authentication tokens refreshed: {} bytes", tokens.len());
            let tokens_str = String::from_utf8_lossy(tokens);
            if tokens_str.is_ascii() && tokens_str.len() < 200 {
                println!("   Tokens: {}", tokens_str);
            }
        })
        .begin()
        .await;

    match session_result {
        Ok(session) => {
            println!("Session created successfully!");
            println!("Session handle: {:?}", session.handle());
            
            println!("\nTesting session operations...");
            
            if session.is_valid() {
                println!("Session handle is valid");
            } else {
                println!("Session handle appears to be invalid");
            }
            
            println!("Ending session...");
            if let Err(e) = session.end().await {
                println!("Warning: Failed to end session cleanly: {}", e);
            } else {
                println!("Session ended successfully");
            }
        },
        Err(e) => {
            println!("Failed to create session: {}", e);
            
            match e {
                proton_sdk_rs::sessions::SessionError::SdkError(sdk_err) => {
                    println!("SDK Error Details: {}", sdk_err);
                },
                proton_sdk_rs::sessions::SessionError::OperationFailed(code) => {
                    println!("SDK operation failed with code: {}", code);
                    match code {
                        -1 => println!("   Possible causes: Invalid credentials, network issues, or SDK not initialized"),
                        401 => println!("   Authentication failed - check username/password"),
                        403 => println!("   Access forbidden - account may be locked or suspended"),
                        422 => println!("   Invalid request format"),
                        _ => println!("   Unknown error code: {}", code),
                    }
                },
                proton_sdk_rs::sessions::SessionError::ProtobufError(proto_err) => {
                    println!("Protobuf Error: {}", proto_err);
                },
                _ => {
                    println!("Other error: {}", e);
                }
            }
        }
    }
    
    Ok(())
}