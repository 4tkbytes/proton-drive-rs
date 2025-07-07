use std::{error::Error, io::{self, Write}};

use proton_sdk_rs::sessions::{SessionBuilder, SessionPlatform};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    println!("Proton Drive Testing App, do not use with real credentials YET");
    let username = "aPriestImamAndRabbiWalkIntoABar";
    let password = "The bartender asks, `What is this, a joke?`";

    println!("Creating session for user: {}", username);

    let session_result = SessionBuilder::new(username, password)
        .with_app_version(SessionPlatform::Windows, "proton-drive-rs", "1.0.0")
        .with_request_response_callback(|data| {
            println!("📡 HTTP: {} bytes", data.len());
        })
        .with_secret_requested_callback(|| {
            print!("🔐 2FA/Secret required. Enter 'y' to continue: ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            input.trim().to_lowercase() == "y"
        })
        .with_tokens_refreshed_callback(|_| {
            println!("🔄 Authentication tokens refreshed");
        })
        .begin()
        .await;

    match session_result {
        Ok(session) => {
            println!("✅ Session created successfully!");
            println!("📋 Session handle: {:?}", session.handle());
            
            println!("\n🧪 Testing session operations...");
            
            // You could add more operations here:
            // - Register keys
            // - Create drive client
            // - etc.
            
            println!("🛑 Ending session...");
            if let Err(e) = session.end().await {
                println!("⚠️  Warning: Failed to end session cleanly: {}", e);
            } else {
                println!("✅ Session ended successfully");
            }
        },
        Err(e) => {
            println!("❌ Failed to create session: {}", e);
            
            match e {
                proton_sdk_rs::sessions::SessionError::SdkError(_) => {
                    println!("💡 Make sure the Proton SDK library is available");
                    println!("Error: {}", e)
                },
                proton_sdk_rs::sessions::SessionError::OperationFailed(code) => {
                    println!("💡 SDK operation failed with code: {}", code);
                    if code == -1 {
                        println!("   This might be due to invalid credentials");
                    }
                },
                _ => {}
            }
        }
    }
    
    Ok(())
}