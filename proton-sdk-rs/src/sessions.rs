use std::{ffi::c_void, fmt, sync::{Arc, Mutex}};

use proton_sdk_sys::{cancellation::CancellationToken, data::{AsyncCallback, BooleanCallback, ByteArray, Callback}, protobufs::{AddressKeyRegistrationRequest, ProtonClientOptions, SessionBeginRequest, SessionRenewRequest, SessionResumeRequest, ToByteArray}, sessions::{self, SessionHandle}};

use crate::cancellation::{self, CancellationTokenSource};

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

pub struct SessionCallbacks {
    pub request_response: Option<RequestResponseCallback>,
    pub secret_requested: Option<SecretRequestedCallback>,
    pub tokens_refreshed: Option<TokensRefreshedCallback>,
}

struct CallbackData {
    request_response: Option<RequestResponseCallback>,
    secret_requested: Option<SecretRequestedCallback>,
    tokens_refreshed: Option<TokensRefreshedCallback>,
    completion_sender: Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<Result<SessionHandle, SessionError>>>>>,
}

impl Default for SessionCallbacks {
    fn default() -> Self {
        Self {
            request_response: None,
            secret_requested: None,
            tokens_refreshed: None,
        }
    }
}

pub struct Session {
    handle: SessionHandle,
    _callback_data: Option<Box<CallbackData>>
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
    pub fn register_armored_locked_user_key(
        &self,
        armored_key: &[u8]
    ) -> Result<(), SessionError> {
        if self.handle.is_null() {
            return Err(SessionError::NullHandle);
        }

        let key_data = ByteArray::from_slice(armored_key);
        let result = sessions::raw::session_register_armored_locked_user_key(self.handle, key_data)?;
        
        if result != 0 {
            return Err(SessionError::OperationFailed(result));
        }
        
        Ok(())
    }

    /// Registers address keys
    pub fn register_address_keys(&self, request: &AddressKeyRegistrationRequest) -> Result<(), SessionError> {
        if self.handle.is_null() {
            return Err(SessionError::NullHandle);
        }

        let proto_buf = request.to_proto_buffer()?;
        let result = sessions::raw::session_register_address_keys(self.handle, proto_buf.as_byte_array())?;
        
        if result != 0 {
            return Err(SessionError::OperationFailed(result));
        }
        
        Ok(())
    }

    /// Ends the session in an async way
    pub async fn end(&self) -> Result<(), SessionError> {
        if self.handle.is_null() {
            return Err(SessionError::NullHandle)
        }

        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(std::sync::Mutex::new(Some(tx)));

        extern "C" fn end_success_callback(state: *const c_void, _response: ByteArray) {
            if !state.is_null() {
                unsafe {
                    let tx = &*(state as *const Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<Result<(), SessionError>>>>>);
                    if let Ok(mut guard) = tx.lock() {
                        if let Some(sender) = guard.take() {
                            let _ = sender.send(Ok(()));
                        }
                    }
                }
            }
        }

        extern "C" fn end_failure_callback(state: *const c_void, error_data: ByteArray) {
            if !state.is_null() {
                unsafe {
                    let tx = &*(state as *const Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<Result<(), SessionError>>>>>);
                    if let Ok(mut guard) = tx.lock() {
                        if let Some(sender) = guard.take() {
                            let _ = sender.send(Err(SessionError::OperationFailed(-1)));
                        }
                    }
                }
            }
        }

        let async_callback = AsyncCallback::new(
            tx.as_ref() as *const _ as *const c_void,
            Some(end_success_callback),
            Some(end_failure_callback),
            CancellationToken::NONE.raw(),
        );

        unsafe {
            let result = sessions::raw::session_end(self.handle, async_callback)?;
            if result != 0 {
                return Err(SessionError::OperationFailed(result));
            }
        }

        rx.await.map_err(|_| SessionError::Cancelled)?
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
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
    pub fn new(username: &str, password: &str) -> Self {
        let request = SessionBeginRequest {
            username: username.to_string(),
            password: password.to_string(),
            options: Some(ProtonClientOptions::default())
        };

        Self {
            request,
            callbacks: SessionCallbacks::default()
        }
    }

    /// Adds options to client session
    pub fn with_options(mut self, options: ProtonClientOptions) -> Self {
        self.request.options = Some(options);
        self
    }

    /// Adds app version according to Proton Semantic Versioning (github)
    pub fn with_app_version(mut self, platform: SessionPlatform, app_name: &str, app_version: &str) -> Self {
        if let Some(ref mut options) = self.request.options {
            let version = format!("{}-drive-{}@{}", platform, app_name, app_version);
            options.app_version = version.to_string();
        }
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

    /// Sets tokens refreshed callback
    pub fn with_tokens_refreshed_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[u8]) + Send + Sync + 'static,
    {
        self.callbacks.tokens_refreshed = Some(Box::new(callback));
        self
    }

    pub async fn begin(self) 
    -> Result<Session, SessionError> 
    {
        let proto_buf = self.request.to_proto_buffer()?;

        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(std::sync::Mutex::new(Some(tx)));

        let callback_data = Box::new(CallbackData {
            request_response: self.callbacks.request_response,
            secret_requested: self.callbacks.secret_requested,
            tokens_refreshed: self.callbacks.tokens_refreshed,
            completion_sender: tx.clone(),
        });

        let callback_ptr = callback_data.as_ref() as *const CallbackData as *const c_void;

        // creating c callbacks
        let request_callback = Callback::new(callback_ptr, Some(request_response_c_callback));
        let secret_callback = BooleanCallback::new(callback_ptr, Some(secret_requested_c_callback));
        let tokens_callback = Callback::new(callback_ptr, Some(tokens_refreshed_c_callback));

        let cancellation_token = CancellationTokenSource::new()    
            .map_err(|e| SessionError::SdkError(e))?;

        // success callback
        extern "C" fn session_success_callback(state: *const c_void, response: ByteArray) {
            if !state.is_null() {
                unsafe {
                    let data = &*(state as *const CallbackData);
                    if let Ok(mut guard) = data.completion_sender.lock() {
                        if let Some(sender) = guard.take() {
                            println!("Session success callback hit!");
                            // TODO: Parse the actual session handle from response
                            // For now, assuming a successful response means we got a handle
                            let _ = sender.send(Ok(SessionHandle::from(12345))); // Replace with actual parsing
                        }
                    }
                }
            }
        }

        // failure callback
        extern "C" fn session_failure_callback(state: *const c_void, error_data: ByteArray) {
            if !state.is_null() {
                unsafe {
                    let data = &*(state as *const CallbackData);
                    if let Ok(mut guard) = data.completion_sender.lock() {
                        if let Some(sender) = guard.take() {
                            println!("Session failure callback hit!");
                            let _ = sender.send(Err(SessionError::OperationFailed(-1)));
                        }
                    }
                }
            }
        }

        let async_callback = AsyncCallback::new(
            callback_ptr,
            Some(session_success_callback),
            Some(session_failure_callback),
            cancellation_token.handle().raw()
        );

        unsafe {
            let result = sessions::raw::session_begin(
                0,
                proto_buf.as_byte_array(),
                request_callback,
                secret_callback,
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
        })
    }

    // Resumes an existing session
    pub async fn resume_session(
        request: SessionResumeRequest,
        callbacks: SessionCallbacks
    ) -> Result<Session, SessionError> {
        let proto_buf = request.to_proto_buffer()?;

        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let callback_data = Box::new(CallbackData {
            request_response: callbacks.request_response,
            secret_requested: callbacks.secret_requested,
            tokens_refreshed: callbacks.tokens_refreshed,
            completion_sender: tx,
        });

        let callback_ptr = callback_data.as_ref() as *const CallbackData as *const c_void;

        let request_callback = Callback::new(callback_ptr, Some(request_response_c_callback));
        let secret_callback = BooleanCallback::new(callback_ptr, Some(secret_requested_c_callback));
        let tokens_callback = Callback::new(callback_ptr, Some(tokens_refreshed_c_callback));

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
                tokens_refreshed: Some(callback),
                completion_sender: Arc::new(std::sync::Mutex::new(None)),
            }))
        } else {
            None
        };

        let callback_ptr = callback_data.as_ref()
            .map(|data| data.as_ref() as *const CallbackData as *const c_void)
            .unwrap_or(std::ptr::null());

        let tokens_callback = Callback::new(callback_ptr, Some(tokens_refreshed_c_callback));

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
            })
        }
    }
}

// C callback implementations that bridge to Rust closures
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

pub enum SessionPlatform {
    Windows,
    macOS,
    Android,
    iOS,
    Linux
}

impl fmt::Display for SessionPlatform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionPlatform::Windows => write!(f, "windows"),
            SessionPlatform::macOS => write!(f, "macos"),
            SessionPlatform::Android => write!(f, "android"),
            SessionPlatform::iOS => write!(f, "ios"),
            SessionPlatform::Linux => write!(f, "linux")
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::sessions::{SessionBuilder, SessionPlatform};

    #[tokio::test]
    async fn test_session_builder() {
        let session_result = SessionBuilder::new(
            "aPriestImamAndRabbiWalkIntoABar@protonmail.com", 
            "The bartender asks, `What is this, a joke?`"
        )
            .with_app_version(SessionPlatform::Windows, "proton-sdk-rs-testing", "0.1.0")
            .with_request_response_callback(|data| {
                println!("Request/Response: {:?}", data);
            })
            .with_secret_requested_callback(|| {
                println!("Secret requested");
                true
            })
            .begin()
            .await;
        
    }
}