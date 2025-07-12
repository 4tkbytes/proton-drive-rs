use std::{env, io};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use log::{debug, error, info, trace, warn};
use proton_sdk_rs::sessions::{Session, SessionBuilder, SessionCallbacks, SessionPlatform};
use proton_sdk_rs::{FromByteArray, ProtonClientOptions, SessionInfo, SessionResumeRequest};
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
    let password_clone2 = password.clone();

    let session_info = File::open("session_info.bin")
        .ok()
        .and_then(|mut f| {
            let mut info_bytes = Vec::new();
            f.read_to_end(&mut info_bytes).ok()?;
            SessionInfo::from_bytes(&info_bytes).ok()
        });

    if let Some(info) = session_info {
        let username_for_2fa = info.username.clone();

        info!("Attempting to resume session...");
        let resume_result = SessionBuilder::resume_session(
            SessionResumeRequest {
                session_id: info.session_id.clone(),
                username: info.username.clone(),
                user_id: info.user_id.clone(),
                access_token: info.access_token.clone(),
                refresh_token: info.refresh_token.clone(),
                scopes: info.scopes.clone(),
                is_waiting_for_second_factor_code: info.is_waiting_for_second_factor_code,
                password_mode: info.password_mode,
                options: Some(ProtonClientOptions::default()),
            },
            SessionCallbacks {
                request_response: Some(Box::new(|data| {
                    let data_str = String::from_utf8_lossy(data);
                    trace!("HTTP: {} bytes", data.len());
                    trace!("Content: {}", data_str);
                })),
                secret_requested: None,
                two_factor_requested: Some(Box::new({
                    let username_for_2fa = username_for_2fa.clone();
                    move |_context| {
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
                                println!("Your data password is the password used to unlock your data. \nIf you do not know what that is or don't have one, just leave it blank and we won't prompt you.");
                                io::stdout().flush().ok();
                                let data_pass = rpassword::prompt_password("Data password: ").unwrap();
                                if !data_pass.trim().is_empty() {
                                    Some(proton_sdk_sys::protobufs::StringResponse {
                                        value: data_pass.trim().to_string(),
                                    })
                                } else {
                                    Some(proton_sdk_sys::protobufs::StringResponse {
                                        value: password.clone(),
                                    })
                                }
                            }
                        };

                        (code_opt, data_pass_opt)
                    }
                })),
                tokens_refreshed: None,
            },
        SessionPlatform::Linux, "proton-drive-rs", "0.1.0");
        match resume_result.await {
            Ok(session) => {
                let data_password = match env::var("NO_DATA_PASS").as_deref() {
                    Ok("true") => {
                        warn!("Data password not provided, setting as password");
                        password_clone2.clone()
                    }
                    _ => {
                        println!("Enter your data password to unlock your data (leave blank to use username): ");
                        io::stdout().flush().ok();
                        let data_pass = rpassword::prompt_password("Data password: ").unwrap();
                        if !data_pass.trim().is_empty() {
                            data_pass.trim().to_string()
                        } else {
                            password_clone2.clone()
                        }
                    }
                };

                // Apply the data password to the session
                session.apply_data_password(&data_password)
                    .map_err(|e| {
                        error!("Failed to apply data password: {}", e);
                        e
                    }).ok();

                info!("Session resumed successfully!");
                return (session, first_run, info.username.clone());
            },
            Err(e) => {
                warn!("Session resume failed [{}], will try creating new session.", e);
            }
        }
    }

    let password_for_2fa = password_clone.clone();
    let session_result = SessionBuilder::new(username.clone(), password_clone.clone())
        .with_app_version(SessionPlatform::Linux, "proton-drive-rs", "0.1.0")
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
                        value: password_for_2fa.clone(),
                    })
                }
                _ => {
                    println!("Your data password is the password used to unlock your data. \n If you do not know what that is or don't have one, just leave it blank and we won't prompt you. ");
                    io::stdout().flush().ok();
                    let data_pass = rpassword::prompt_password("Data password: ").unwrap();
                    if !data_pass.trim().is_empty() {
                        Some(proton_sdk_sys::protobufs::StringResponse {
                            value: data_pass.trim().to_string(),
                        })
                    } else {
                        Some(proton_sdk_sys::protobufs::StringResponse {
                            value: password_for_2fa.clone(),
                        })
                    }
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