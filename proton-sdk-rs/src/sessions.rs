use std::{
    ffi::c_void,
    fmt,
    sync::{Arc, Mutex},
};

use log::{debug, error, info, trace, warn};
use proton_sdk_sys::{
    data::{AsyncCallback, BooleanCallback, ByteArray, Callback},
    protobufs::{
        AddressKeyRegistrationRequest, ProtonClientOptions, SessionBeginRequest,
        SessionRenewRequest, SessionResumeRequest, ToByteArray,
    },
    sessions::{self, SessionHandle},
};
use proton_sdk_sys::protobufs::StringResponse;
use crate::cancellation::CancellationToken;
use proton_sdk_sys::protobufs::SessionInfo;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("SDK error: {0}")]
    SdkError(#[from] anyhow::Error),

    #[error("Session operation failed")]
    OperationFailed(i32),

    #[error("Protobuf error: {0}")]
    ProtobufError(#[from] proton_sdk_sys::protobufs::ProtoError),

    #[error("Session handle is null")]
    NullHandle,

    #[error("Operation was cancelled")]
    Cancelled,
}

pub type RequestResponseCallback = Box<dyn Fn(&[u8]) + Send + Sync>;
pub type SecretRequestedCallback = Box<dyn Fn() -> bool + Send + Sync>;
pub type TokensRefreshedCallback = Box<dyn Fn(&[u8]) + Send + Sync>;
pub type TwoFactorRequestedCallbackRust = Box<dyn Fn(&[u8]) -> (
    Option<StringResponse>, Option<StringResponse>
) + Send + Sync>;

pub struct SessionCallbacks {
    pub request_response: Option<RequestResponseCallback>,
    pub secret_requested: Option<SecretRequestedCallback>,
    pub two_factor_requested: Option<TwoFactorRequestedCallbackRust>,
    pub tokens_refreshed: Option<TokensRefreshedCallback>,
}

struct CallbackData {
    request_response: Option<RequestResponseCallback>,
    secret_requested: Option<SecretRequestedCallback>,
    two_factor_requested: Option<TwoFactorRequestedCallbackRust>,
    tokens_refreshed: Option<TokensRefreshedCallback>,
    completion_sender: Arc<
        std::sync::Mutex<Option<tokio::sync::oneshot::Sender<Result<SessionHandle, SessionError>>>>,
    >,
}

impl Default for SessionCallbacks {
    fn default() -> Self {
        Self {
            request_response: None,
            secret_requested: Some(Box::new(|| {
                log::debug!("Session requested");
                true
            })),
            two_factor_requested: None,
            tokens_refreshed: None,
        }
    }
}

pub struct Session {
    handle: SessionHandle,
    _callback_data: Option<Box<CallbackData>>,
    cancellation_token: CancellationToken,
}

impl Session {
    /// Returns the session handle
    pub fn handle(&self) -> SessionHandle {
        self.handle
    }

    /// Checks if the session is null
    pub fn is_valid(&self) -> bool {
        !self.handle.is_null()
    }

    /// Registers an armored locked user key??
    pub fn register_armored_locked_user_key(&self, armored_key: &[u8]) -> Result<(), SessionError> {
        if self.handle.is_null() {
            return Err(SessionError::NullHandle);
        }

        let key_data = ByteArray::from_slice(armored_key);
        let result =
            sessions::raw::session_register_armored_locked_user_key(self.handle, key_data)?;

        if result != 0 {
            return Err(SessionError::OperationFailed(result));
        }

        Ok(())
    }

    /// Registers address keys
    pub fn register_address_keys(
        &self,
        request: &AddressKeyRegistrationRequest,
    ) -> Result<(), SessionError> {
        if self.handle.is_null() {
            return Err(SessionError::NullHandle);
        }

        let proto_buf = request.to_proto_buffer()?;
        let result =
            sessions::raw::session_register_address_keys(self.handle, proto_buf.as_byte_array())?;

        if result != 0 {
            return Err(SessionError::OperationFailed(result));
        }

        Ok(())
    }

    pub fn info(&self) -> anyhow::Result<SessionInfo> {
        let session = sessions::raw::session_get_info(
            self.handle(), 
            self.cancellation_token().handle()
        ).map_err(|e| SessionError::SdkError(e))?;
        
        #[cfg(debug_assertions)]
        {
            trace!("SessionId: {:?}", session.session_id);
            trace!("Username: {}", session.username);
            trace!("UserID: {:?}", session.user_id);
            trace!("Access Token: {:?}", session.access_token);
            trace!("Refresh Token: {:?}", session.refresh_token);
            trace!("Scopes: ");
            for scope in &session.scopes {
                trace!("    {:?}", scope);
            }
            trace!("Is waiting for second factor code: {}", session.is_waiting_for_second_factor_code);
            trace!("Password mode: {}", session.password_mode().as_str_name());
        }

        Ok(session)
    }

    /// Ends the session ~~in an async way (breaks func)~~
    pub fn end(&self) -> Result<(), SessionError> {
        if self.handle.is_null() {
            return Err(SessionError::NullHandle);
        }

        debug!("Ending session synchronously...");
        debug!("Session handle: {:?}", self.handle);

        unsafe {
            match sessions::raw::session_free(self.handle) {
                Ok(_t) => {
                    debug!("Session freed successfully");
                    Ok(())
                }
                Err(e) => {
                    error!("Session free failed: {}", e);
                    Err(SessionError::SdkError(e))
                }
            }
        }
    }

    pub fn cancellation_token(&self) -> &CancellationToken {
        &self.cancellation_token
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                // todo: save the token information and write to a file before discarding session
                let _ = sessions::raw::session_free(self.handle);
            }
        }
    }
}

pub struct SessionBuilder {
    request: SessionBeginRequest,
    callbacks: SessionCallbacks,
}

impl SessionBuilder {
    /// Creates a new Proton account session
    pub fn new(username: String, password: String) -> Self {
        let request = SessionBeginRequest {
            username: username,
            password: password,
            two_factor_code: None,
            options: Some(ProtonClientOptions::default()),
        };

        Self {
            request,
            callbacks: SessionCallbacks::default(),
        }
    }

    /// Adds options to client session
    pub fn with_options(mut self, options: ProtonClientOptions) -> Self {
        self.request.options = Some(options);
        self
    }

    /// Adds app version according to Proton Semantic Versioning (github)
    pub fn with_app_version(
        mut self,
        platform: SessionPlatform,
        app_name: &str,
        app_version: &str,
    ) -> Self {
        if let Some(ref mut options) = self.request.options {
            let version = format!("external-drive-{}_{}@{}", app_name, platform, app_version);
            options.app_version = version.to_string();
        }
        info!(
            "App version: external-drive-{}_{}@{}", app_name, platform, app_version
        );
        self
    }

    pub fn with_rclone_app_version_spoof(mut self) -> Self {
        if let Some(ref mut options) = self.request.options {
            options.app_version = "macos-drive@1.0.0-alpha.1+proton-sdk-sys".to_string();
        }
        debug!("App version: macos-drive@1.0.0-alpha.1+proton-sdk-sys");
        self
    }

    /// Sets request/response callback
    pub fn with_request_response_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[u8]) + Send + Sync + 'static,
    {
        self.callbacks.request_response = Some(Box::new(callback));
        self
    }

    /// Sets secret requested callback
    pub fn with_secret_requested_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        self.callbacks.secret_requested = Some(Box::new(callback));
        self
    }

    /// Sets two factor requested callback
    pub fn with_two_factor_requested_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[u8]) -> (Option<StringResponse>, Option<StringResponse>) + Send + Sync + 'static,
    {
        self.callbacks.two_factor_requested = Some(Box::new(callback));
        self
    }

    /// Sets tokens refreshed callback
    pub fn with_tokens_refreshed_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[u8]) + Send + Sync + 'static,
    {
        self.callbacks.tokens_refreshed = Some(Box::new(callback));
        self
    }

    pub async fn begin(self) -> Result<Session, SessionError> {
        let censor = |input: &String, censor: char| {
            let mut temp = String::new();
            for len in 0..input.len()-2 {
                temp.push(censor);
            }
            temp
        };

        debug!("Creating session for user: {}", self.request.username);
        debug!(
            "Using credentials: username={}, password={}chars",
            format!("{}{}{}", self.request.username.chars().next().unwrap(), censor(&self.request.username, '*'), self.request.username.chars().last().unwrap()),
            self.request.password.len()
        );

        let proto_buf = self.request.to_proto_buffer()?;

        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(std::sync::Mutex::new(Some(tx)));

        let callback_data = Box::new(CallbackData {
            request_response: self.callbacks.request_response,
            secret_requested: self.callbacks.secret_requested,
            two_factor_requested: self.callbacks.two_factor_requested,
            tokens_refreshed: self.callbacks.tokens_refreshed,
            completion_sender: tx.clone(),
        });
        let callback_ptr = callback_data.as_ref() as *const CallbackData as *const c_void;

        // creating c callbacks
        let request_callback = Callback::new(callback_ptr, Some(request_response_c_callback));
        let secret_callback = BooleanCallback::new(callback_ptr, Some(secret_requested_c_callback));
        let two_factor_callback = proton_sdk_sys::data::TwoFactorRequestedCallback::new(
            callback_ptr,
            Some(two_factor_requested_c_callback),
        );
        let tokens_callback = Callback::new(callback_ptr, Some(tokens_refreshed_c_callback));

        let cancellation_token = CancellationToken::new().map_err(|e| SessionError::SdkError(e))?;

        // success callback
        extern "C" fn session_success_callback(state: *const c_void, response: ByteArray) {
            if !state.is_null() {
                unsafe {
                    let data = &*(state as *const CallbackData);
                    if let Ok(mut guard) = data.completion_sender.lock() {
                        if let Some(sender) = guard.take() {
                            debug!("Session success callback hit!");

                            let response_slice = response.as_slice();
                            trace!("Success response: {} bytes", response_slice.len());

                            // Debug: Show response content
                            if response_slice.len() <= 100 {
                                trace!("Response hex: {:02x?}", response_slice);
                                if let Ok(response_str) = std::str::from_utf8(response_slice) {
                                    trace!("Response as string: {}", response_str);
                                }
                            }

                            // Parse session handle
                            let session_handle = unsafe { parse_session_handle(&response) }
                                .unwrap_or_else(|e| {
                                    warn!("Warning: {}, using default handle", e);
                                    SessionHandle::from(1) // Non-zero to indicate success
                                });

                            debug!("Using session handle: {:?}", session_handle);
                            let _ = sender.send(Ok(session_handle));
                        }
                    }
                }
            } else {
                error!("Callback state is null!");
            }
        }

        // failure callback
        extern "C" fn session_failure_callback(state: *const c_void, error_data: ByteArray) {
            if !state.is_null() {
                unsafe {
                    let data = &*(state as *const CallbackData);
                    debug!("Session failure callback hit!");

                    let (error_code, error_message) = parse_sdk_error(&error_data);
                    error!(
                        "Error details: code={}, message={}",
                        error_code, error_message
                    );

                    match error_code {
                        401 => error!("Authentication failed - check username/password"),
                        403 => error!("Access forbidden - account may be suspended"),
                        422 => error!("Invalid request - check your input data"),
                        429 => error!("Rate limited - try again later"),
                        1000..=1999 => error!("Client error - check your request format"),
                        2000..=2999 => error!("Server error - Proton service may be down"),
                        _ => error!("Check network connectivity and credentials"),
                    }

                    if let Ok(mut guard) = data.completion_sender.lock() {
                        if let Some(sender) = guard.take() {
                            let _ = sender.send(Err(SessionError::OperationFailed(error_code)));
                        }
                    }
                }
            }
        }

        let async_callback = AsyncCallback::new(
            callback_ptr,
            Some(session_success_callback),
            Some(session_failure_callback),
            cancellation_token.handle().raw(),
        );

        unsafe {
            let result = sessions::raw::session_begin(
                0,
                proto_buf.as_byte_array(),
                request_callback,
                secret_callback,
                two_factor_callback,
                tokens_callback,
                async_callback,
            )?;

            if result != 0 {
                return Err(SessionError::OperationFailed(result));
            }
        }

        let session_handle = rx.await.map_err(|_| SessionError::Cancelled)??;

        Ok(Session {
            handle: session_handle,
            _callback_data: Some(callback_data),
            cancellation_token,
        })
    }

    // Resumes an existing session
    pub async fn resume_session(
        request: SessionResumeRequest,
        callbacks: SessionCallbacks,
    ) -> Result<Session, SessionError> {
        let proto_buf = request.to_proto_buffer()?;

        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let callback_data = Box::new(CallbackData {
            request_response: callbacks.request_response,
            secret_requested: callbacks.secret_requested,
            two_factor_requested: callbacks.two_factor_requested,
            tokens_refreshed: callbacks.tokens_refreshed,
            completion_sender: tx,
        });

        let callback_ptr = callback_data.as_ref() as *const CallbackData as *const c_void;

        let request_callback = Callback::new(callback_ptr, Some(request_response_c_callback));
        let secret_callback = BooleanCallback::new(callback_ptr, Some(secret_requested_c_callback));
        let tokens_callback = Callback::new(callback_ptr, Some(tokens_refreshed_c_callback));

        let cancellation_token = CancellationToken::new().map_err(|e| SessionError::SdkError(e))?;

        unsafe {
            let (result, session_handle) = sessions::raw::session_resume(
                proto_buf.as_byte_array(),
                request_callback,
                secret_callback,
                tokens_callback,
            )?;

            if result != 0 {
                return Err(SessionError::OperationFailed(result));
            }

            Ok(Session {
                handle: session_handle,
                _callback_data: Some(callback_data),
                cancellation_token,
            })
        }
    }

    /// Renew an existing session
    pub async fn renew_session(
        old_session: &Session,
        request: SessionRenewRequest,
        tokens_refreshed_callback: Option<TokensRefreshedCallback>,
    ) -> Result<Session, SessionError> {
        if old_session.handle.is_null() {
            return Err(SessionError::NullHandle);
        }

        let proto_buf = request.to_proto_buffer()?;

        let callback_data = if let Some(callback) = tokens_refreshed_callback {
            Some(Box::new(CallbackData {
                request_response: None,
                secret_requested: None,
                two_factor_requested: None,
                tokens_refreshed: Some(callback),
                completion_sender: Arc::new(std::sync::Mutex::new(None)),
            }))
        } else {
            None
        };

        let callback_ptr = callback_data
            .as_ref()
            .map(|data| data.as_ref() as *const CallbackData as *const c_void)
            .unwrap_or(std::ptr::null());

        let tokens_callback = Callback::new(callback_ptr, Some(tokens_refreshed_c_callback));

        let cancellation_token = old_session.cancellation_token.clone();

        unsafe {
            let (result, new_session_handle) = sessions::raw::session_renew(
                old_session.handle,
                proto_buf.as_byte_array(),
                tokens_callback,
            )?;

            if result != 0 {
                return Err(SessionError::OperationFailed(result));
            }

            Ok(Session {
                handle: new_session_handle,
                _callback_data: callback_data,
                cancellation_token,
            })
        }
    }
}

unsafe fn parse_session_handle(response: &ByteArray) -> Result<SessionHandle, String> {
    let response_slice = response.as_slice();

    if response_slice.is_empty() {
        return Err("Empty response".to_string());
    }

    trace!("Response data: {} bytes", response_slice.len());

    // Try to parse as protobuf IntResponse first
    use proton_sdk_sys::protobufs::FromByteArray;
    if let Ok(int_response) = proton_sdk_sys::protobufs::IntResponse::from_byte_array(response) {
        trace!("Parsed as IntResponse: value = {}", int_response.value);
        return Ok(SessionHandle::from(int_response.value as isize));
    }

    // Try to parse as protobuf SessionTokens
    if let Ok(session_tokens) = proton_sdk_sys::protobufs::SessionTokens::from_byte_array(response)
    {
        trace!("Parsed as SessionTokens - using access token hash as handle");
        let handle_value = session_tokens
            .access_token
            .as_bytes()
            .iter()
            .fold(0i64, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as i64));
        return Ok(SessionHandle::from(handle_value as isize));
    }

    // Try to interpret as raw bytes (lil indian)
    if response_slice.len() >= 8 {
        let handle_bytes = [
            response_slice[0],
            response_slice[1],
            response_slice[2],
            response_slice[3],
            response_slice[4],
            response_slice[5],
            response_slice[6],
            response_slice[7],
        ];
        let handle_value = i64::from_le_bytes(handle_bytes);
        println!("Parsed as raw i64: {}", handle_value);
        return Ok(SessionHandle::from(handle_value as isize));
    }

    // Try to interpret as raw bytes (big indian)
    if response_slice.len() >= 8 {
        let handle_bytes = [
            response_slice[0],
            response_slice[1],
            response_slice[2],
            response_slice[3],
            response_slice[4],
            response_slice[5],
            response_slice[6],
            response_slice[7],
        ];
        let handle_value = i64::from_be_bytes(handle_bytes);
        trace!("Parsed as raw i64 (big-endian): {}", handle_value);
        return Ok(SessionHandle::from(handle_value as isize));
    }

    // Try as string that might contain a number
    if let Ok(response_str) = std::str::from_utf8(response_slice) {
        if let Ok(handle_value) = response_str.trim().parse::<isize>() {
            trace!("Parsed as string number: {}", handle_value);
            return Ok(SessionHandle::from(handle_value));
        }
    }

    if response_slice.len() <= 50 {
        trace!("Response hex dump: {:02x?}", response_slice);
    } else {
        trace!(
            "Response hex dump (first 50 bytes): {:02x?}",
            &response_slice[..50]
        );
    }

    Err(format!(
        "Could not parse session handle from {} bytes",
        response_slice.len()
    ))
}

extern "C" fn request_response_c_callback(state: *const c_void, data: ByteArray) {
    if !state.is_null() {
        unsafe {
            let callback_data = &*(state as *const CallbackData);
            if let Some(ref callback) = callback_data.request_response {
                let slice = data.as_slice();
                callback(slice);
            }
        }
    }
}

extern "C" fn secret_requested_c_callback(state: *const c_void, _data: ByteArray) -> bool {
    if !state.is_null() {
        unsafe {
            let callback_data = &*(state as *const CallbackData);
            if let Some(ref callback) = callback_data.secret_requested {
                return callback();
            }
        }
    }
    false
}

extern "C" fn tokens_refreshed_c_callback(state: *const c_void, data: ByteArray) {
    if !state.is_null() {
        unsafe {
            let callback_data = &*(state as *const CallbackData);
            if let Some(ref callback) = callback_data.tokens_refreshed {
                let slice = data.as_slice();
                callback(slice);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn proton_sdk_free(ptr: *mut u8) {
    if !ptr.is_null() {
        unsafe { Box::from_raw(ptr); }
    }
}

extern "C" fn two_factor_requested_c_callback(
    state: *const c_void,
    context: ByteArray,
    out_code: *mut ByteArray,
    data_pass: *mut ByteArray,
) -> bool {
    if !state.is_null() {
        unsafe {
            let callback_data = &*(state as *const CallbackData);
            if let Some(ref callback) = callback_data.two_factor_requested {
                let input = context.as_slice();
                let (code_opt, pass_opt) = callback(input);
                let mut any_set = false;

                if !out_code.is_null() {
                    if let Some(code) = code_opt {
                        if let Ok(proto_buf) = code.to_proto_buffer() {
                            let bytes = proto_buf.as_byte_array();
                            let vec = std::slice::from_raw_parts(bytes.pointer, bytes.length).to_vec();
                            let boxed = vec.into_boxed_slice();
                            let ptr = Box::into_raw(boxed) as *const u8;
                            *out_code = ByteArray {
                                pointer: ptr,
                                length: bytes.length,
                            };
                            trace!("Allocated out_code at {:p} ({} bytes)", ptr, bytes.length);
                            any_set = true;
                        }
                    }
                }

                if !data_pass.is_null() {
                    if let Some(pass) = pass_opt {
                        if let Ok(proto_buf) = pass.to_proto_buffer() {
                            let bytes = proto_buf.as_byte_array();
                            let vec = std::slice::from_raw_parts(bytes.pointer, bytes.length).to_vec();
                            let boxed = vec.into_boxed_slice();
                            let ptr = Box::into_raw(boxed) as *const u8;
                            *data_pass = ByteArray {
                                pointer: ptr,
                                length: bytes.length,
                            };
                            trace!("Allocated data_pass at {:p} ({} bytes)", ptr, bytes.length);
                            any_set = true;
                        }
                    }
                }

                return any_set;
            }
        }
    }
    false
}

pub enum SessionPlatform {
    Windows,
    #[allow(non_camel_case_types)]
    macOS,
    Android,
    #[allow(non_camel_case_types)]
    iOS,
    Linux,
}

impl fmt::Display for SessionPlatform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionPlatform::Windows => write!(f, "windows"),
            SessionPlatform::macOS => write!(f, "macos"),
            SessionPlatform::Android => write!(f, "android"),
            SessionPlatform::iOS => write!(f, "ios"),
            SessionPlatform::Linux => write!(f, "linux"),
        }
    }
}

fn parse_sdk_error(error_data: &ByteArray) -> (i32, String) {
    unsafe {
        let error_slice = error_data.as_slice();

        if error_slice.is_empty() {
            return (-1, "Unknown error - no details provided".to_string());
        }

        // Try protobuf Error first
        use proton_sdk_sys::protobufs::FromByteArray;
        if let Ok(error_proto) = proton_sdk_sys::protobufs::Error::from_byte_array(error_data) {
            return (error_proto.primary_code() as i32, error_proto.message);
        }

        // Try as UTF-8 string
        if let Ok(error_str) = std::str::from_utf8(error_slice) {
            // Check if it's JSON
            if error_str.starts_with('{') {
                return (-1, format!("JSON Error: {}", error_str));
            }
            return (-1, error_str.to_string());
        }

        // Last resort: hex dump
        if error_slice.len() <= 50 {
            return (-1, format!("Binary error data: {:02x?}", error_slice));
        } else {
            return (
                -1,
                format!(
                    "Binary error data ({} bytes): {:02x?}...",
                    error_slice.len(),
                    &error_slice[..20]
                ),
            );
        }
    }
}
