// // Response: IntResponse
// int uploader_create(
//     intptr_t client_handle,
//     ByteArray pointer, // FileUploaderCreationRequest
//     AsyncCallback callback
// );

// // Response: FileNode
// int uploader_upload_file_or_revision(
//     intptr_t uploader_handle,
//     ByteArray pointer, // FileUploadRequest
//     AsyncCallbackWithProgress callback
// );

// // Response: Revision
// int uploader_upload_revision(
//     intptr_t uploader_handle,
//     ByteArray pointer, // RevisionUploadRequest
//     AsyncCallbackWithProgress callback
// );

// void uploader_free(intptr_t uploader_handle);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploaderHandle(pub isize);

impl UploaderHandle {
    /// Creates a new handle as null
    pub fn null() -> Self {
        Self(0)
    }

    /// Checks if the handle is null
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// Returns the raw handle
    pub fn raw(&self) -> isize {
        self.0
    }
}

impl From<isize> for UploaderHandle {
    fn from(value: isize) -> Self {
        Self(value)
    }
}

pub mod raw {
    use crate::{
        data::{AsyncCallback, AsyncCallbackWithProgress, ByteArray, Callback},
        drive::DriveClientHandle,
        uploads::{self, UploaderHandle},
        ProtonSDKLib,
    };

    /// Creates a new uploader
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `request` - FileUploaderCreationRequest as ByteArray
    /// * `callback` - Async callback for completion with uploader handle
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    /// Response: IntResponse (uploader handle) delivered via success callback
    pub fn uploader_create(
        client_handle: DriveClientHandle,
        request: ByteArray,
        callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let create_uploader_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallback) -> i32,
            > = sdk.sdk_library.get(b"uploader_create")?;

            let result = create_uploader_fn(client_handle.raw(), request, callback);

            Ok(result)
        }
    }

    /// Uploads a file or creates a new file revision
    ///
    /// # Parameters
    /// * `uploader_handle` - Handle to the uploader
    /// * `request` - FileUploadRequest as ByteArray
    /// * `callback` - Async callback with progress reporting for upload completion
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    /// Response: FileNode delivered via success callback
    pub fn uploader_upload_file_or_revision(
        uploader_handle: UploaderHandle,
        request: ByteArray,
        callback: AsyncCallbackWithProgress,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let upload_file_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallbackWithProgress) -> i32,
            > = sdk.sdk_library.get(b"uploader_upload_file_or_revision")?;

            let result = upload_file_fn(uploader_handle.raw(), request, callback);

            Ok(result)
        }
    }

    /// Uploads a new revision of an existing file
    ///
    /// # Parameters
    /// * `uploader_handle` - Handle to the uploader
    /// * `request` - RevisionUploadRequest as ByteArray
    /// * `callback` - Async callback with progress reporting for upload completion
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    /// Response: Revision delivered via success callback
    pub fn uploader_upload_revision(
        uploader_handle: UploaderHandle,
        request: ByteArray,
        callback: AsyncCallbackWithProgress,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let upload_revision_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallbackWithProgress) -> i32,
            > = sdk.sdk_library.get(b"uploader_upload_revision")?;

            let result = upload_revision_fn(uploader_handle.raw(), request, callback);

            Ok(result)
        }
    }

    /// Frees uploader resources
    ///
    /// # Parameters
    /// * `uploader_handle` - Handle to the uploader to free
    pub fn uploader_free(uploader_handle: UploaderHandle) -> anyhow::Result<()> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let free_uploader_fn: libloading::Symbol<unsafe extern "C" fn(isize)> =
                sdk.sdk_library.get(b"uploader_free")?;

            free_uploader_fn(uploader_handle.raw());
            Ok(())
        }
    }
}
