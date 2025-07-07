use std::{ffi::c_void, fmt};

use log::{debug, warn};
use proton_sdk_sys::{data::{AsyncCallback, ByteArray}, observability::{self, ObservabilityHandle}, sessions::SessionHandle};

use crate::cancellation::CancellationToken;

#[derive(Debug, thiserror::Error)]
pub enum ObservabilityError {
    #[error("SDK error: {0}")]
    SdkError(#[from] anyhow::Error),

    #[error("Observability service start failed with code: {0}")]
    StartFailed(i32),

    #[error("Observability data flush failed: {0}")]
    FlushFailed(String),

    #[error("Observability flush operation timed out")]
    FlushTimeout,

    #[error("Observability service handle is null")]
    NullHandle,

    #[error("Invalid session handle")]
    InvalidSession,
}

/// Service that sends basic telemetry from Proton Drive and Proton Accounts
pub struct ObservabilityService {
    handle: ObservabilityHandle,
    _session: SessionHandle
}

impl ObservabilityService {
    /// Creates a new observability service for the given session
    /// 
    /// # Arguments
    /// * `session` - The active session handle
    /// 
    /// # Returns
    /// A new ObservabilityService instance or an error if creation failed
    pub fn new(session: SessionHandle) -> Result<Self, ObservabilityError> {
        if session.is_null() {
            return Err(ObservabilityError::InvalidSession);
        }

        let (result, obs_handle) = observability::raw::observability_service_start_new(session)
            .map_err(|e| ObservabilityError::SdkError(e))?;

        if result != 0 {
            return Err(ObservabilityError::StartFailed(result));
        }

        if obs_handle.is_null() {
            return Err(ObservabilityError::NullHandle);
        }

        log::debug!("Observability service started with handle: {:?}", obs_handle);

        Ok(Self {
            handle: obs_handle,
            _session: session,
        })
    }

    pub fn handle(&self) -> ObservabilityHandle {
        self.handle
    }

    pub fn is_valid(&self) -> bool {
        !self.handle.is_null()
    }

    /// Flushes observability data asynchronously
    /// 
    /// This sends any pending telemetry data to Proton's servers.
    /// 
    /// # Arguments
    /// * `cancellation_token` - Token to cancel the operation if needed
    /// 
    /// # Returns
    /// Ok(()) on success, or an error if the flush failed
    pub async fn flush(&self, cancellation_token: &CancellationToken) -> Result<(), ObservabilityError> {
        if self.handle.is_null() {
            return Err(ObservabilityError::NullHandle);
        }

        let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), ObservabilityError>>();
        let tx_ptr = Box::leak(Box::new(tx));

        extern "C" fn flush_success_callback(state: *const c_void, _response: ByteArray) {
            log::debug!("Flush success callback hit!");
            if !state.is_null() {
                unsafe {
                    let tx_ptr = state as *mut tokio::sync::oneshot::Sender<Result<(), ObservabilityError>>;
                    let tx = Box::from_raw(tx_ptr);
                    let _ = tx.send(Ok(()));
                }
            }
        }

        extern "C" fn flush_failure_callback(state: *const c_void, error_data: ByteArray) {
            log::debug!("Flush failure callback hit...");
            if !state.is_null() {
                unsafe {
                    let tx_ptr = state as *mut tokio::sync::oneshot::Sender<Result<(), ObservabilityError>>;
                    let tx = Box::from_raw(tx_ptr);
                    
                    let error_slice = error_data.as_slice();
                    let error_msg = if error_slice.is_empty() {
                        "Unknown flush error".to_string()
                    } else {
                        String::from_utf8_lossy(error_slice).to_string()
                    };
                    
                    let _ = tx.send(Err(ObservabilityError::FlushFailed(error_msg)));
                }
            }
        }

        let async_callback = AsyncCallback::new(
            tx_ptr as *mut _ as *const std::ffi::c_void,
            Some(flush_success_callback),
            Some(flush_failure_callback),
            cancellation_token.handle().raw()
        );

        let result = observability::raw::observability_service_flush(self.handle, async_callback)
            .map_err(|e| ObservabilityError::SdkError(e))?;

        if result != 0 {
            unsafe { let _ = Box::from_raw(tx_ptr); }
            return Err(ObservabilityError::FlushFailed(format!("FFI call failed with code: {}", result)));
        }

        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(result) => result.map_err(|e| ObservabilityError::FlushFailed(e.to_string()))?,
            Err(_) => Err(ObservabilityError::FlushTimeout),
        }
    }

    /// Explicitly frees the observability service
    /// 
    /// Note: This is automatically called when the ObservabilityService is dropped,
    /// so you usually don't need to call this manually.
    pub fn free(self) -> Result<(), ObservabilityError> {
        if !self.handle.is_null() {
            observability::raw::observability_service_free(self.handle)
                .map_err(|e| ObservabilityError::SdkError(e))?;
            log::debug!("Observability service freed successfully");
        }
        Ok(())
    }
}

impl fmt::Debug for ObservabilityService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservabilityService")
            .field("handle", &self.handle)
            .field("valid", &self.is_valid())
            .finish()
    }
}

impl Drop for ObservabilityService {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            if let Err(e) = observability::raw::observability_service_free(self.handle) {
                warn!("Failed to free observability service in Drop: {}", e);
            } else {
                debug!("Observability service cleaned up automatically");
            }
        }
    }
}

pub struct ObservabilityServiceBuilder {
    session: SessionHandle,
}

impl ObservabilityServiceBuilder {
    /// Creates a new ObservabilityService builder
    pub fn new(session: SessionHandle) -> Self {
        Self { session }
    }

    /// Builds the ObservabilityService
    pub fn build(self) -> Result<ObservabilityService, ObservabilityError> {
        ObservabilityService::new(self.session)
    }
}

/// A wrapper to the ObservabilityService struct
/// 
/// This struct allows you to enable or disable telemetry for Proton Services. 
pub struct OptionalObservability(Option<ObservabilityService>);

impl OptionalObservability {
    /// Creates an enabled observability service
    pub fn enabled(session: SessionHandle) -> Result<Self, ObservabilityError> {
        Ok(Self(Some(ObservabilityService::new(session)?)))
    }

    /// Creates a disabled observability service (no-op)
    pub fn disabled() -> Self {
        Self(None)
    }

    /// Gets the handle if observability is enabled, otherwise returns null handle
    pub fn handle(&self) -> ObservabilityHandle {
        self.0.as_ref()
            .map(|obs| obs.handle())
            .unwrap_or_else(ObservabilityHandle::null)
    }

    /// Checks if observability is enabled and valid
    pub fn is_enabled(&self) -> bool {
        self.0.as_ref().map(|obs| obs.is_valid()).unwrap_or(false)
    }

    /// Flushes data if observability is enabled
    pub async fn flush_if_enabled(&self, cancellation_token: &CancellationToken) -> Result<(), ObservabilityError> {
        if let Some(obs) = &self.0 {
            obs.flush(cancellation_token).await
        } else {
            Ok(())
        }
    }
}

impl fmt::Debug for OptionalObservability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(obs) => f.debug_tuple("OptionalObservability").field(obs).finish(),
            None => f.debug_tuple("OptionalObservability").field(&"Disabled").finish(),
        }
    }
}
