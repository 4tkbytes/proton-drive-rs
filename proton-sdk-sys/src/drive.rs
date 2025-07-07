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
    use crate::data::AsyncCallback;

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

    // int drive_client_get_volumes(
    //     intptr_t client_handle,
    //     ByteArray empty_request,
    //     AsyncCallback callback
    // );
    /// Gets all volumes for the Drive client
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `empty_request` - Empty ByteArray (unused)
    /// * `callback` - Async callback for the response
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn drive_client_get_volumes(
        client_handle: DriveClientHandle,
        empty_request: ByteArray,
        callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let get_volumes_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallback) -> i32,
            > = sdk.sdk_library.get(b"drive_client_get_volumes")?;

            let result = get_volumes_fn(client_handle.raw(), empty_request, callback);
            Ok(result)
        }
    }

    // int drive_client_get_share(
    //     intptr_t client_handle,
    //     ByteArray share_request, // ShareId
    //     AsyncCallback callback
    // );
    /// Gets a specific share by ID
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `share_request` - ShareId as ByteArray
    /// * `callback` - Async callback for the response
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn drive_client_get_share(
        client_handle: DriveClientHandle,
        share_request: ByteArray,
        callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let get_share_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallback) -> i32,
            > = sdk.sdk_library.get(b"drive_client_get_share")?;

            let result = get_share_fn(client_handle.raw(), share_request, callback);
            Ok(result)
        }
    }

    // int drive_client_get_folder_children(
    //     intptr_t client_handle,
    //     ByteArray folder_request, // FolderChildrenRequest
    //     AsyncCallback callback
    // );
    /// Gets children of a folder
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `folder_request` - FolderChildrenRequest as ByteArray
    /// * `callback` - Async callback for the response
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn drive_client_get_folder_children(
        client_handle: DriveClientHandle,
        folder_request: ByteArray,
        callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let get_children_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallback) -> i32,
            > = sdk.sdk_library.get(b"drive_client_get_folder_children")?;

            let result = get_children_fn(client_handle.raw(), folder_request, callback);
            Ok(result)
        }
    }

    // int drive_client_create_folder(
    //     intptr_t client_handle,
    //     ByteArray folder_request, // FolderCreationRequest
    //     AsyncCallback callback
    // );
    /// Creates a new folder
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `folder_request` - FolderCreationRequest as ByteArray
    /// * `callback` - Async callback for the response
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn drive_client_create_folder(
        client_handle: DriveClientHandle,
        folder_request: ByteArray,
        callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let create_folder_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallback) -> i32,
            > = sdk.sdk_library.get(b"drive_client_create_folder")?;

            let result = create_folder_fn(client_handle.raw(), folder_request, callback);
            Ok(result)
        }
    }

    // int drive_client_move_node(
    //     intptr_t client_handle,
    //     ByteArray move_request, // NodeMoveRequest
    //     AsyncCallback callback
    // );
    /// Moves a node to a different location
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `move_request` - NodeMoveRequest as ByteArray
    /// * `callback` - Async callback for the response
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn drive_client_move_node(
        client_handle: DriveClientHandle,
        move_request: ByteArray,
        callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let move_node_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallback) -> i32,
            > = sdk.sdk_library.get(b"drive_client_move_node")?;

            let result = move_node_fn(client_handle.raw(), move_request, callback);
            Ok(result)
        }
    }

    // int drive_client_delete_node(
    //     intptr_t client_handle,
    //     ByteArray delete_request, // NodeDeleteRequest
    //     AsyncCallback callback
    // );
    /// Deletes a node (moves to trash)
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `delete_request` - NodeDeleteRequest as ByteArray
    /// * `callback` - Async callback for the response
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn drive_client_delete_node(
        client_handle: DriveClientHandle,
        delete_request: ByteArray,
        callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let delete_node_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallback) -> i32,
            > = sdk.sdk_library.get(b"drive_client_delete_node")?;

            let result = delete_node_fn(client_handle.raw(), delete_request, callback);
            Ok(result)
        }
    }

    // int drive_client_rename_node(
    //     intptr_t client_handle,
    //     ByteArray rename_request, // NodeRenameRequest
    //     AsyncCallback callback
    // );
    /// Renames a node
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `rename_request` - NodeRenameRequest as ByteArray
    /// * `callback` - Async callback for the response
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn drive_client_rename_node(
        client_handle: DriveClientHandle,
        rename_request: ByteArray,
        callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let rename_node_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallback) -> i32,
            > = sdk.sdk_library.get(b"drive_client_rename_node")?;

            let result = rename_node_fn(client_handle.raw(), rename_request, callback);
            Ok(result)
        }
    }

    // int drive_client_get_node_info(
    //     intptr_t client_handle,
    //     ByteArray node_request, // NodeIdentity
    //     AsyncCallback callback
    // );
    /// Gets detailed information about a node
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `node_request` - NodeIdentity as ByteArray
    /// * `callback` - Async callback for the response
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn drive_client_get_node_info(
        client_handle: DriveClientHandle,
        node_request: ByteArray,
        callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let get_node_info_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallback) -> i32,
            > = sdk.sdk_library.get(b"drive_client_get_node_info")?;

            let result = get_node_info_fn(client_handle.raw(), node_request, callback);
            Ok(result)
        }
    }

    // pub fn drive_client_get
}

#[cfg(test)]
mod tests {}
