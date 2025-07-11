use std::{env, io};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use log::{debug, error, info, trace, warn};
use proton_sdk_rs::sessions::{Session, SessionBuilder, SessionPlatform};
use rpassword::prompt_password;

pub async fn create_new_session() -> (Session, bool, String) {
    let first_run = match std::fs::read_to_string(".cfg") {
        Ok(cfg) => !cfg.lines().any(|line| line.trim() == "INITIAL_INDEX=true"),
        Err(_) => true,
    };

    if first_run {
        debug!("First run!");
    }

    if let Err(_) = dotenv::dotenv() {
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let env_path = workspace_root.join(".cfg");
        dotenv::from_path(env_path).ok();
    }

    if let Ok(log_level) = env::var("RUST_LOG") {
        env_logger::init_from_env(env_logger::Env::default().default_filter_or(&log_level));
    } else {
        env_logger::init();
        warn!("No RUST_LOG environment variable found. Setting default log level.");
    }

    let username = env::var("PROTON_USERNAME").unwrap_or_else(|_| {
        print!("Enter your email: ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let username = input.trim().to_string();

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(".cfg")
            .unwrap();
        writeln!(file, "PROTON_USERNAME={}", username).unwrap();

        username
    });

    let password = env::var("PROTON_PASSWORD").unwrap_or_else(|_| {
        io::stdout().flush().unwrap();
        let password = prompt_password("Password: ").unwrap();

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(".cfg")
            .unwrap();
        writeln!(file, "PROTON_PASSWORD={}", password).unwrap();

        password
    });
    let password_clone = password.clone();

    let session_result = SessionBuilder::new(username, password.clone())
        .with_app_version(SessionPlatform::Linux, "proton-drive-rs", "0.1.0")
        // .with_rclone_app_version_spoof()
        .with_request_response_callback(|data| {
            let data_str = String::from_utf8_lossy(data);
            trace!("HTTP: {} bytes", data.len());
            trace!("Content: {}", data_str);
        })
        .with_two_factor_requested_callback(move |_context| {
            print!("Enter 2FA code: ");
            io::stdout().flush().ok();
            let mut code = String::new();
            let code_opt = if io::stdin().read_line(&mut code).is_ok() {
                let code = code.trim();
                if !code.is_empty() {
                    Some(proton_sdk_sys::protobufs::StringResponse {
                        value: code.to_string(),
                    })
                } else {
                    None
                }
            } else {
                None
            };

            let data_pass_opt = match env::var("NO_DATA_PASS").as_deref() {
                Ok("true") => {
                    warn!("Data password not provided, setting as users password");
                    Some(proton_sdk_sys::protobufs::StringResponse {
                        value: password.clone(),
                    })
                }
                _ => {
                    println!("Your data password is the password used to unlock your data. \n If you do not know what that is or don't have one, just leave it blank and we won't prompt you. ");
                    io::stdout().flush().ok();
                    let mut data_pass = rpassword::prompt_password("Data password: ").unwrap();
                    let data_pass_result = if io::stdin().read_line(&mut data_pass).is_ok() {
                        let data_pass = data_pass.trim();
                        let mut file = OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(".cfg")
                            .unwrap();
                        if !data_pass.is_empty() {
                            writeln!(file, "NO_DATA_PASS=false").ok();
                            Some(proton_sdk_sys::protobufs::StringResponse {
                                value: data_pass.to_string(),
                            })
                        } else {
                            writeln!(file, "NO_DATA_PASS=true").ok();
                            Some(proton_sdk_sys::protobufs::StringResponse {
                                value: password.clone(),
                            })
                        }
                    } else {
                        None
                    };
                    data_pass_result
                }
            };

            (code_opt, data_pass_opt)
        })
        .begin()
        .await;

    let session = match session_result {
        Ok(session) => {
            println!("Session created successfully!");
            debug!("Session handle: {:?}", session.handle());
            session
        }
        Err(e) => {
            println!("Failed to create session: {}", e);

            match e {
                proton_sdk_rs::sessions::SessionError::SdkError(sdk_err) => {
                    error!("SDK Error Details: {}", sdk_err);
                }
                proton_sdk_rs::sessions::SessionError::OperationFailed(code) => {
                    error!("SDK operation failed with code: {}", code);
                    match code {
                        -1 => error!(
                            "   Possible causes: Invalid credentials, network issues, or SDK not initialized"
                        ),
                        401 => println!("   Authentication failed - check username/password"),
                        403 => println!("   Access forbidden - account may be locked or suspended"),
                        422 => println!("   Invalid request format"),
                        8002 => println!("   Two factor code failed"),
                        _ => println!("   Unknown error code: {}", code),
                    }
                }
                proton_sdk_rs::sessions::SessionError::ProtobufError(proto_err) => {
                    error!("Protobuf Error: {}", proto_err);
                }
                _ => {
                    error!("Other error: {}", e);
                }
            }
            panic!("Failed to create session");
        }
    };
    (session, first_run, password_clone)
}