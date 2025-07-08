use crate::data::ByteArray;
use crate::observability::ObservabilityHandle;
use crate::sessions::SessionHandle;
use crate::ProtonSDKLib;

/// Handle for a Drive client
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriveClientHandle(pub isize);

impl DriveClientHandle {
    /// Creates a null/invalid handle
    pub fn null() -> Self {
        Self(0)
    }

    /// Checks if the handle is null/invalid
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// Gets the raw isize value for FFI
    pub fn raw(&self) -> isize {
        self.0
    }
}

impl From<isize> for DriveClientHandle {
    fn from(handle: isize) -> Self {
        Self(handle)
    }
}

/// Raw FFI functions for Drive client management
pub mod raw {
    use crate::{cancellation::CancellationTokenHandle, data::{AsyncCallback}};

    use super::*;

    // int drive_client_create(
    //     intptr_t session_handle,
    //     intptr_t observability_handle,
    //     ByteArray pointer, // ProtonDriveClientCreateRequest
    //     intptr_t* out_client_handle
    // );
    /// Creates a new Drive client
    ///
    /// # Parameters
    /// * `session_handle` - Handle to the active session
    /// * `observability_handle` - Handle to the observability service
    /// * `request` - ProtonDriveClientCreateRequest as ByteArray
    ///
    /// # Returns
    /// (Result code, Drive client handle) - code 0 = success, handle for the created client
    pub fn drive_client_create(
        session_handle: SessionHandle,
        observability_handle: ObservabilityHandle,
        request: ByteArray,
    ) -> anyhow::Result<(i32, DriveClientHandle)> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let create_client_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, isize, ByteArray, *mut isize) -> i32,
            > = sdk.sdk_library.get(b"drive_client_create")?;

            let mut client_handle: isize = 0;
            let result = create_client_fn(
                session_handle.raw(),
                observability_handle.raw(),
                request,
                &mut client_handle,
            );

            Ok((result, DriveClientHandle::from(client_handle)))
        }
    }

    // int drive_client_register_node_keys(
    //     intptr_t client_handle,
    //     ByteArray pointer // NodeKeysRegistrationRequest
    // );
    /// Registers node keys with the Drive client
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `request` - NodeKeysRegistrationRequest as ByteArray
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn drive_client_register_node_keys(
        client_handle: DriveClientHandle,
        request: ByteArray,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let register_keys_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray) -> i32,
            > = sdk.sdk_library.get(b"drive_client_register_node_keys")?;

            let result = register_keys_fn(client_handle.raw(), request);

            Ok(result)
        }
    }

    // int drive_client_register_share_key(
    //     intptr_t client_handle,
    //     ByteArray pointer // ShareKeyRegistrationRequest
    // );
    /// Registers a share key with the Drive client
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `request` - ShareKeyRegistrationRequest as ByteArray
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn drive_client_register_share_key(
        client_handle: DriveClientHandle,
        request: ByteArray,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let register_key_fn: libloading::Symbol<unsafe extern "C" fn(isize, ByteArray) -> i32> =
                sdk.sdk_library.get(b"drive_client_register_share_key")?;

            let result = register_key_fn(client_handle.raw(), request);

            Ok(result)
        }
    }

    // void drive_client_free(intptr_t client_handle);
    /// Frees Drive client resources
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client to free
    pub fn drive_client_free(client_handle: DriveClientHandle) -> anyhow::Result<()> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let free_client_fn: libloading::Symbol<unsafe extern "C" fn(isize)> =
                sdk.sdk_library.get(b"drive_client_free")?;

            free_client_fn(client_handle.raw());
            Ok(())
        }
    }

    /// Fetches the available volumes in the client, typically only returning
    /// 1 volume but can change. 
    /// 
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `cancellation_token` - Handle to the cancellation token
    pub fn drive_client_get_volumes(
        client_handle: DriveClientHandle,
        cancellation_token: CancellationTokenHandle,
    ) -> anyhow::Result<ByteArray> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let get_volumes_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, isize) -> ByteArray
            > = sdk.sdk_library.get(b"drive_client_get_volumes")?;

            Ok(get_volumes_fn(client_handle.raw(), cancellation_token.raw()))
        }
    }

    // ByteArray drive_client_get_shares(
    //     intptr_t client_handle,
    //     ByteArray volume_metadata,
    //     intptr_t cancellation_token
    // );
    pub fn drive_client_get_shares(
        client_handle: DriveClientHandle,
        volume_metadata: ByteArray,
        cancellation_token: CancellationTokenHandle
    ) -> anyhow::Result<ByteArray> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let get_shares_fn: libloading::Symbol<unsafe extern "C" fn(isize, ByteArray, isize) -> ByteArray>
            = sdk.sdk_library.get(b"drive_client_get_shares")?;

            Ok(get_shares_fn(client_handle.raw(), volume_metadata, cancellation_token.raw()))
        }
    }
}

#[cfg(test)]
mod tests {}
