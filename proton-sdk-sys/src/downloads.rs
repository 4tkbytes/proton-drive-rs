#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloaderHandle(pub isize);

impl DownloaderHandle {
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

impl From<isize> for DownloaderHandle {
    fn from(handle: isize) -> Self {
        Self(handle)
    }
}

/// Raw FFI functions for download operations
pub mod raw {
    use crate::{
        data::{AsyncCallback, AsyncCallbackWithProgress, ByteArray},
        drive::DriveClientHandle,
        ProtonSDKLib,
    };

    use super::*;

    // int downloader_create(
    //     intptr_t client_handle,
    //     ByteArray pointer, // Empty
    //     AsyncCallback callback
    // );
    /// Creates a new downloader
    ///
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `request` - Empty ByteArray (reserved for future use)
    /// * `callback` - Async callback for completion with downloader handle
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    /// The downloader handle will be provided via the success callback
    pub fn downloader_create(
        client_handle: DriveClientHandle,
        request: ByteArray,
        callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let create_downloader_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallback) -> i32,
            > = sdk.sdk_library.get(b"downloader_create")?;

            let result = create_downloader_fn(client_handle.raw(), request, callback);

            Ok(result)
        }
    }

    // // Response: File
    // int downloader_download_file(
    //     intptr_t downloader_handle,
    //     ByteArray pointer, // FileDownloadRequest
    //     AsyncCallbackWithProgress callback
    // );
    /// Downloads a file
    ///
    /// # Parameters
    /// * `downloader_handle` - Handle to the downloader
    /// * `request` - FileDownloadRequest as ByteArray
    /// * `callback` - Async callback with progress reporting for download completion
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    /// The downloaded file data will be provided via the success callback
    pub fn downloader_download_file(
        downloader_handle: DownloaderHandle,
        request: ByteArray,
        callback: AsyncCallbackWithProgress,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let download_file_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, AsyncCallbackWithProgress) -> i32,
            > = sdk.sdk_library.get(b"downloader_download_file")?;

            let result = download_file_fn(downloader_handle.raw(), request, callback);

            Ok(result)
        }
    }

    // void downloader_free(intptr_t downloader_handle);
    /// Frees downloader resources
    ///
    /// # Parameters
    /// * `downloader_handle` - Handle to the downloader to free
    pub fn downloader_free(downloader_handle: DownloaderHandle) -> anyhow::Result<()> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let free_downloader_fn: libloading::Symbol<unsafe extern "C" fn(isize)> =
                sdk.sdk_library.get(b"downloader_free")?;

            free_downloader_fn(downloader_handle.raw());
            Ok(())
        }
    }
}
