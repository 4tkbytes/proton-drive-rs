#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionHandle(pub isize);

impl SessionHandle {
    /// Sets the session handle to null (or 0)
    pub fn null() -> Self {
        Self(0)
    }

    /// Checks if the function is null
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// Returns the raw value of the function
    pub fn raw(&self) -> isize {
        self.0
    }
}

impl From<isize> for SessionHandle {
    fn from(handle: isize) -> Self {
        Self(handle)
    }
}

pub mod raw {
    use crate::{cancellation::{self, CancellationTokenHandle}, data::*, protobufs::{FromByteArray, SessionInfo}, ProtonSDKLib};

    use super::*;

    // int session_begin(
    //     intptr_t unused_handle, // Added for the sake of uniformity
    //     ByteArray pointer, // SessionBeginRequest
    //     Callback request_response_body_callback,
    //     BooleanCallback secret_requested_callback,
    //     Callback tokens_refreshed_callback,
    //     AsyncCallback callback
    // );
    /// Begins a new session
    ///
    /// # Parameters
    /// * `unused_handle` - Added for uniformity (pass 0)
    /// * `request` - SessionBeginRequest as ByteArray
    /// * `request_response_callback` - Callback for HTTP request/response events
    /// * `secret_requested_callback` - Callback when secrets are needed
    /// * `tokens_refreshed_callback` - Callback when tokens are refreshed
    /// * `async_callback` - Async callback for completion
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub unsafe fn session_begin(
        unused_handle: isize,
        request: ByteArray,
        request_response_callback: Callback,
        secret_requested_callback: BooleanCallback,
        two_factor_requested_callback: TwoFactorRequestedCallback,
        tokens_refreshed_callback: Callback,
        async_callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        let sdk = ProtonSDKLib::instance()?;

        let session_begin_fn: libloading::Symbol<
            unsafe extern "C" fn(
                isize,
                ByteArray,
                Callback,
                BooleanCallback,
                TwoFactorRequestedCallback,
                Callback,
                AsyncCallback,
            ) -> i32,
        > = sdk.sdk_library.get(b"session_begin")?;

        let result = session_begin_fn(
            unused_handle,
            request,
            request_response_callback,
            secret_requested_callback,
            two_factor_requested_callback,
            tokens_refreshed_callback,
            async_callback,
        );

        Ok(result)
    }

    // int session_resume(
    //     ByteArray pointer, // SessionResumeRequest
    //     Callback request_response_body_callback,
    //     BooleanCallback secret_requested_callback,
    //     Callback tokens_refreshed_callback,
    //     intptr_t* session_handle // TODO: SessionResumeResponse
    // );
    /// Resumes an existing session
    ///
    /// # Parameters
    /// * `request` - SessionResumeRequest as ByteArray
    /// * `request_response_callback` - Callback for HTTP request/response events
    /// * `secret_requested_callback` - Callback when secrets are needed
    /// * `tokens_refreshed_callback` - Callback when tokens are refreshed
    ///
    /// # Returns
    /// (Result code, Session handle) - code 0 = success, handle for the resumed session
    pub unsafe fn session_resume(
        request: ByteArray,
        request_response_callback: Callback,
        secret_requested_callback: BooleanCallback,
        tokens_refreshed_callback: Callback,
    ) -> anyhow::Result<(i32, SessionHandle)> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let session_resume_fn: libloading::Symbol<
                unsafe extern "C" fn(
                    ByteArray,
                    Callback,
                    BooleanCallback,
                    Callback,
                    *mut isize,
                ) -> i32,
            > = sdk.sdk_library.get(b"session_resume")?;

            let mut session_handle: isize = 0;
            let result = session_resume_fn(
                request,
                request_response_callback,
                secret_requested_callback,
                tokens_refreshed_callback,
                &mut session_handle,
            );

            Ok((result, SessionHandle::from(session_handle)))
        }
    }

    // int session_renew(
    //     intptr_t old_session_handle,
    //     ByteArray pointer, // SessionRenewRequest
    //     Callback tokens_refreshed_callback,
    //     intptr_t* new_session_handle // TODO: SessionRenewResponse
    // );
    /// Renews an existing session
    ///
    /// # Parameters
    /// * `old_session_handle` - Handle to the session to renew
    /// * `request` - SessionRenewRequest as ByteArray
    /// * `tokens_refreshed_callback` - Callback when tokens are refreshed
    ///
    /// # Returns
    /// (Result code, New session handle) - code 0 = success, handle for the new session
    pub unsafe fn session_renew(
        old_session_handle: SessionHandle,
        request: ByteArray,
        tokens_refreshed_callback: Callback,
    ) -> anyhow::Result<(i32, SessionHandle)> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let session_renew_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, Callback, *mut isize) -> i32,
            > = sdk.sdk_library.get(b"session_renew")?;

            let mut new_session_handle: isize = 0;
            let result = session_renew_fn(
                old_session_handle.raw(),
                request,
                tokens_refreshed_callback,
                &mut new_session_handle,
            );

            Ok((result, SessionHandle::from(new_session_handle)))
        }
    }

    // int session_end(
    //     intptr_t session_handle, // Todo: SessionEndRequest
    //     AsyncCallback callback
    // );
    /// Ends a session
    ///
    /// # Parameters
    /// * `session_handle` - Handle to the session to end
    /// * `async_callback` - Async callback for completion
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub unsafe fn session_end(
        session_handle: SessionHandle,
        async_callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let session_end_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, AsyncCallback) -> i32,
            > = sdk.sdk_library.get(b"session_end")?;

            let result = session_end_fn(session_handle.raw(), async_callback);

            Ok(result)
        }
    }

    // void session_free(intptr_t session_handle);
    /// Frees session resources
    ///
    /// # Parameters
    /// * `session_handle` - Handle to the session to free
    pub unsafe fn session_free(session_handle: SessionHandle) -> anyhow::Result<()> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let session_free_fn: libloading::Symbol<unsafe extern "C" fn(isize)> =
                sdk.sdk_library.get(b"session_free")?;

            session_free_fn(session_handle.raw());
            Ok(())
        }
    }

    // int session_register_armored_locked_user_key(
    //     intptr_t session_handle,
    //     ByteArray armoredUserKey
    // );
    /// Registers an armored locked user key with the session
    ///
    /// # Parameters
    /// * `session_handle` - Handle to the active session
    /// * `armored_user_key` - The armored (PGP/ASCII armored) locked user key as ByteArray
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn session_register_armored_locked_user_key(
        session_handle: SessionHandle,
        armored_user_key: ByteArray,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let register_key_fn: libloading::Symbol<unsafe extern "C" fn(isize, ByteArray) -> i32> =
                sdk.sdk_library
                    .get(b"session_register_armored_locked_user_key")?;

            let result = register_key_fn(session_handle.raw(), armored_user_key);

            Ok(result)
        }
    }

    // int session_register_address_keys(
    //     intptr_t session_handle,
    //     ByteArray pointer // AddressKeyRegistrationRequest
    // );
    /// Registers address keys with the session
    ///
    /// # Parameters
    /// * `session_handle` - Handle to the active session
    /// * `request` - AddressKeyRegistrationRequest as ByteArray
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn session_register_address_keys(
        session_handle: SessionHandle,
        request: ByteArray,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let register_keys_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray) -> i32,
            > = sdk.sdk_library.get(b"session_register_address_keys")?;

            let result = register_keys_fn(session_handle.raw(), request);

            Ok(result)
        }
    }

    /// Fetches the session info such as related tokens. 
    /// 
    /// # Parameters
    /// * `session_handle` - The handle to the session
    /// * `cancellation_token` - Cancellation token
    /// 
    /// # Returns
    /// The `SessionInfo` protobuf
    pub fn session_get_info(session_handle: SessionHandle, cancellation_token: CancellationTokenHandle) -> anyhow::Result<crate::protobufs::SessionInfo> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;
            let session_get_info_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, isize, *mut ByteArray) -> i32,
            > = sdk.sdk_library.get(b"session_get_info")?;

            let mut out_bytes = ByteArray::empty();
            let result = session_get_info_fn(session_handle.raw(), cancellation_token.raw(), &mut out_bytes as *mut _);
            if result != 0 {
                anyhow::bail!("session_get_info failed with code {}", result);
            }

            let info = SessionInfo::from_byte_array(&out_bytes)?;

            Ok(info)
        }
    }

    pub fn session_apply_data_password(
        session_handle: SessionHandle,
        password: ByteArray,
        cancellation_token: CancellationTokenHandle
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let apply_data_password_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, ByteArray, isize) -> i32
            > = sdk.sdk_library.get(b"session_apply_data_password")?;

            let result = apply_data_password_fn(
                session_handle.raw(),
                password,
                cancellation_token.raw(),
            );

            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {}
