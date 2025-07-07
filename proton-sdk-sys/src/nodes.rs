pub mod raw {
    use crate::{data::{AsyncCallback, ByteArray}, drive::DriveClientHandle, ProtonSDKLib};

    use super::*;
    
    // int node_decrypt_armored_name(
    //     intptr_t client_handle,
    //     ByteArray pointer,
    //     AsyncCallback callback
    // );
    /// Decrypts an armored node name
    /// 
    /// # Parameters
    /// * `client_handle` - Handle to the Drive client
    /// * `request` - Request data as ByteArray (likely contains encrypted name data)
    /// * `callback` - Async callback for completion with decrypted name result
    /// 
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn node_decrypt_armored_name(
        client_handle: DriveClientHandle,
        request: ByteArray,
        callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;
            
            let decrypt_name_fn: libloading::Symbol<unsafe extern "C" fn(
                isize,
                ByteArray,
                AsyncCallback,
            ) -> i32> = sdk.sdk_library.get(b"node_decrypt_armored_name")?;
            
            let result = decrypt_name_fn(
                client_handle.raw(),
                request,
                callback,
            );
            
            Ok(result)
        }
    }
}