use std::fmt;

use log::{debug, warn};
use proton_sdk_sys::{drive::{self, DriveClientHandle}, observability::{self, ObservabilityHandle}, protobufs::{NodeKeysRegistrationRequest, ProtonDriveClientCreateRequest, ShareKeyRegistrationRequest, ToByteArray}, sessions::SessionHandle};

use crate::{observability::ObservabilityService, sessions::Session};

pub struct DriveClient {
    handle: DriveClientHandle,
    _session: SessionHandle
}

#[derive(Debug, thiserror::Error)]
pub enum DriveError {
    #[error("SDK error: {0}")]
    SdkError(#[from] anyhow::Error),

    #[error("Protobuf error: {0}")]
    ProtobufError(#[from] proton_sdk_sys::protobufs::ProtoError),

    #[error("Drive client creation failed with code: {0}")]
    CreationFailed(i32),

    #[error("Operation '{operation}' failed with code: {code}")]
    OperationFailed { operation: String, code: i32 },

    #[error("Drive client handle is null")]
    NullHandle,

    #[error("Invalid session handle")]
    InvalidSession,
}

impl DriveClient {
    /// Creates a new Drive client for the given session
    /// 
    /// # Arguments
    /// * `session` - The active session handle
    /// * `observability` - The observability handle (use ObservabilityHandle::null() if not needed)
    /// * `request` - Configuration for the Drive client
    /// 
    /// # Returns
    /// A new DriveClient instance or an error if creation failed
    pub fn new(
        session: SessionHandle,
        observability: ObservabilityHandle,
        request: ProtonDriveClientCreateRequest
    ) -> Result<Self, DriveError> {
        if session.is_null() {
            return Err(DriveError::InvalidSession)
        }

        let proto_buf = request.to_proto_buffer()
            .map_err(|e| DriveError::ProtobufError(e))?;

        let (result, client_handle) = drive::raw::drive_client_create(session, observability, proto_buf.as_byte_array())
            .map_err(|e| DriveError::SdkError(e))?;
        
        if result != 0 {
            return Err(DriveError::CreationFailed(result));
        }

        if client_handle.is_null() {
            return Err(DriveError::NullHandle);
        }

        debug!("Drive client created with handle: {:?}", client_handle);

        Ok(Self {
            handle: client_handle,
            _session: session,
        })
    }

    /// Fetches and returns the handle of DriveClient
    pub fn handle(&self) -> DriveClientHandle {
        self.handle
    }

    /// Checks if the handle is valid (not null)
    pub fn is_valid(&self) -> bool {
        !self.handle.is_null()
    }

    /// Registers node keys with the Drive client
    /// 
    /// Node keys are used for encrypting/decrypting file content and metadata
    /// 
    /// # Arguments
    /// * `request` - The node keys registration request
    /// 
    /// # Returns
    /// Ok(()) on success, or an error if registration failed
    pub fn register_node_keys(&self, request: NodeKeysRegistrationRequest) -> Result<(), DriveError> {
        if self.handle.is_null() {
            return Err(DriveError::NullHandle);
        }

        let proto_buf = request.to_proto_buffer().map_err(|e| DriveError::ProtobufError((e)))?;

        let result = drive::raw::drive_client_register_node_keys(
            self.handle,
            proto_buf.as_byte_array()
        ).map_err(|e| DriveError::SdkError(e))?;

        if result != 0 {
            return Err(DriveError::OperationFailed { 
                operation: "register_node_keys".to_string(),
                code: result
            });
        }

        debug!("Node keys registered successfully");
        Ok(())
    }

    /// Registers a share key with the Drive client
    /// 
    /// Share keys are used for sharing files and folders between users
    /// 
    /// # Arguments
    /// * `request` - The share key registration request
    /// 
    /// # Returns
    /// Ok(()) on success, or an error if registration failed
    pub fn register_share_key(&self, request: ShareKeyRegistrationRequest) -> Result<(), DriveError> {
        if self.handle.is_null() {
            return Err(DriveError::NullHandle);
        }

        let proto_buf = request.to_proto_buffer()
            .map_err(|e| DriveError::ProtobufError(e))?;

        let result = drive::raw::drive_client_register_share_key(
            self.handle,
            proto_buf.as_byte_array(),
        ).map_err(|e| DriveError::SdkError(e))?;

        if result != 0 {
            return Err(DriveError::OperationFailed {
                operation: "register_share_key".to_string(),
                code: result
            });
        }

        debug!("Share key registered successfully");
        Ok(())
    }

    pub fn free(self) -> Result<(), DriveError> {
        Ok(if !self.handle.is_null() {
            drive::raw::drive_client_free(self.handle)
                .map_err(|e| DriveError::SdkError(e))?;
            debug!("Drive client freed successfully!")
        })
    }
}

impl fmt::Debug for DriveClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DriveClient")
            .field("handle", &self.handle)
            .field("valid", &self.is_valid())
            .finish()
    }
}

impl Drop for DriveClient {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            if let Err(e) = drive::raw::drive_client_free(self.handle) {
                warn!("Failed to free Drive client in Drop: {}", e);
            } else {
                debug!("Drive client cleaned up automatically");
            }
        }
    }
}

pub struct DriveClientBuilder {
    session: SessionHandle,
    observability: ObservabilityHandle,
    request: ProtonDriveClientCreateRequest
}

impl DriveClientBuilder {
    /// Builds a new DriveClient
    pub fn new(session: Session) -> Self {
        Self {
            session: session.handle(),
            observability: ObservabilityHandle::null(),
            request: ProtonDriveClientCreateRequest::default(),
        }
    }

    /// Sets the observability handle
    pub fn with_observability(mut self, observability: ObservabilityService) -> Self {
        self.observability = observability.handle();
        self
    }

    /// Sets the Drive client creation request
    pub fn with_request(mut self, request: ProtonDriveClientCreateRequest) -> Self {
        self.request = request;
        self
    }

    /// Builds it
    pub fn build(self) -> Result<DriveClient, DriveError> {
        DriveClient::new(self.session, self.observability, self.request)
    }
}