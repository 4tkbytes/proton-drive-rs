use std::{ffi::c_void, fmt};

use log::{debug, error, warn};
use proton_sdk_sys::{
    cancellation, data::ByteArray, drive::{self, DriveClientHandle}, observability::{self, ObservabilityHandle}, protobufs::{
        NodeIdentity, NodeKeysRegistrationRequest, NodeType, NodeTypeList, ProtonDriveClientCreateRequest, Share, ShareKeyRegistrationRequest, ToByteArray, VolumeEventType, VolumeMetadata, VolumesResponse
    }, sessions::SessionHandle
};

use proton_sdk_sys::prost::Message;

use crate::{cancellation::CancellationToken, observability::ObservabilityService, sessions::Session};

pub struct DriveClient {
    handle: DriveClientHandle,
    session: Session,
}

#[derive(Debug, thiserror::Error)]
pub enum DriveError {
    #[error("SDK error: {0}")]
    SdkError(#[from] anyhow::Error),

    #[error("Protobuf error: {0}")]
    ProtobufError(#[from] proton_sdk_sys::protobufs::ProtoError),

    #[error("Volume error: {0}")]
    VolumeError(anyhow::Error),

    #[error("Share error: {0}")]
    ShareError(anyhow::Error),

    #[error("Node operation failed with error: {0}")]
    NodeError(anyhow::Error),

    #[error("The function returned an empty byte array")]
    EmptyByteArray,

    #[error("Drive client creation failed with code: {0}")]
    CreationFailed(i32),

    #[error("Operation '{operation}' failed with code: {code}")]
    OperationFailed { operation: String, code: i32 },

    #[error("Operation '{operation}' failed")]
    OperationFailedWithoutCode { operation: String},

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
        session: Session,
        observability: ObservabilityHandle,
        request: ProtonDriveClientCreateRequest,
    ) -> Result<Self, DriveError> {
        if session.handle().is_null() {
            return Err(DriveError::InvalidSession);
        }

        let proto_buf = request
            .to_proto_buffer()
            .map_err(|e| DriveError::ProtobufError(e))?;

        let (result, client_handle) =
            drive::raw::drive_client_create(session.handle(), observability, proto_buf.as_byte_array())
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
            session,
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

    pub fn session(&self) -> &Session {
        &self.session
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
    pub fn register_node_keys(
        &self,
        request: NodeKeysRegistrationRequest,
    ) -> Result<(), DriveError> {
        if self.handle.is_null() {
            return Err(DriveError::NullHandle);
        }

        let proto_buf = request
            .to_proto_buffer()
            .map_err(|e| DriveError::ProtobufError((e)))?;

        let result =
            drive::raw::drive_client_register_node_keys(self.handle, proto_buf.as_byte_array())
                .map_err(|e| DriveError::SdkError(e))?;

        if result != 0 {
            return Err(DriveError::OperationFailed {
                operation: "register_node_keys".to_string(),
                code: result,
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
    pub fn register_share_key(
        &self,
        request: ShareKeyRegistrationRequest,
    ) -> Result<(), DriveError> {
        if self.handle.is_null() {
            return Err(DriveError::NullHandle);
        }

        let proto_buf = request
            .to_proto_buffer()
            .map_err(|e| DriveError::ProtobufError(e))?;

        let result =
            drive::raw::drive_client_register_share_key(self.handle, proto_buf.as_byte_array())
                .map_err(|e| DriveError::SdkError(e))?;

        if result != 0 {
            return Err(DriveError::OperationFailed {
                operation: "register_share_key".to_string(),
                code: result,
            });
        }

        debug!("Share key registered successfully");
        Ok(())
    }

    pub async fn get_volumes(&self) -> Result<Vec<VolumeMetadata>, DriveError> {
        let handle = self.handle;
        let cancellation_token = self.session.cancellation_token().handle();

        let bytes = tokio::task::spawn_blocking(move || {
            let result = drive::raw::drive_client_get_volumes(
                handle,
                cancellation_token)
                .map_err(|e| DriveError::SdkError(e))?;

            if result.is_empty() {
                return Err(DriveError::EmptyByteArray);
            }

            let bytes = unsafe {
                result.as_slice().to_vec()
            };

            Ok(bytes)
        }).await.map_err(|e| DriveError::SdkError(anyhow::Error::new(e)))?;

        let bytes = bytes?;
        let response = match VolumesResponse::decode(&*bytes) {
                Ok(value) => value,
                Err(error) => return Err(DriveError::ProtobufError(error.into()))
            };

        Ok(response.volumes)
    }

    pub async fn get_shares(&self, volume_metadata: &VolumeMetadata) -> Result<Share, DriveError> {
        let handle = self.handle;
        let token = self.session.cancellation_token().handle();
        let metadata_vec = volume_metadata.encode_to_vec();

        let bytes = tokio::task::spawn_blocking(move || {
            let metadata = ByteArray::from_slice(&metadata_vec);
            let result = drive::raw::drive_client_get_shares(
                handle, 
                metadata,
                token
            ).map_err(|e| DriveError::ShareError(e))?;

            if result.is_empty() {
                return Err(DriveError::EmptyByteArray);
            }

            let bytes = unsafe {
                result.as_slice().to_vec()
            };

            Ok(bytes)
        }).await.map_err(|e| DriveError::ShareError(anyhow::Error::new(e)))?;

        let bytes = bytes?;
        let response = match Share::decode(&*bytes) {
            Ok(value) => value,
            Err(error) => return Err(DriveError::ProtobufError(error.into())),
        };

        Ok(response)
    }

    pub async fn get_folder_children(&self, node_identity: NodeIdentity) -> Result<Vec<NodeType>, DriveError> {
        let handle = self.handle;
        let token = self.session.cancellation_token().handle();
        let identity_vec = node_identity.encode_to_vec();

        let bytes = tokio::task::spawn_blocking(move || {
            let identity = ByteArray::from_slice(&identity_vec);
            let result = drive::raw::drive_client_get_folder_children(
                handle, 
                identity, 
                token
            ).map_err(|e| DriveError::NodeError(anyhow::anyhow!(e)))?;

            if result.is_empty() {
                return Err(DriveError::EmptyByteArray);
            }

            let bytes = unsafe { result.as_slice().to_vec() };
            Ok(bytes)
        }).await.map_err(|e| DriveError::NodeError(anyhow::anyhow!(e)))?;

        let bytes = bytes?;
       
        let node_list = NodeTypeList::decode(&*bytes)
            .map_err(|e| DriveError::ProtobufError(e.into()))?;

        Ok(node_list.nodes)
    }

    /// Manually frees up the Proton Drive client handles in memory
    pub fn free(self) -> Result<(), DriveError> {
        Ok(if !self.handle.is_null() {
            drive::raw::drive_client_free(self.handle).map_err(|e| DriveError::SdkError(e))?;
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
    session: Session,
    observability: ObservabilityHandle,
    request: ProtonDriveClientCreateRequest,
}

impl DriveClientBuilder {
    /// Builds a new DriveClient
    pub fn new(session: Session) -> Self {
        Self {
            session: session,
            observability: ObservabilityHandle::null(),
            request: ProtonDriveClientCreateRequest::default(),
        }
    }

    /// Sets the observability handle
    pub fn with_observability(mut self, observability: ObservabilityHandle) -> Self {
        self.observability = observability;
        self
    }

    /// Sets the Drive client creation request
    pub fn with_request(mut self, request: ProtonDriveClientCreateRequest) -> Self {
        self.request = request;
        self
    }

    /// Builds it
    pub fn build(self) -> Result<DriveClient, DriveError> {
        if self.request.client_id.is_none() {
            error!(
                "Unable to locate client id. Please add in a client id (just the name of your app)"
            );
            error!("May fail without it, carrying on...");
        }
        DriveClient::new(self.session, self.observability, self.request)
    }
}
