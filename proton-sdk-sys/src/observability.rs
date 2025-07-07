#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservabilityHandle(isize);

impl ObservabilityHandle {
    /// Creates a null handle
    pub fn null() -> Self {
        Self(0)
    }

    /// Checks if a handle is null
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// Returns the raw handle
    pub fn raw(&self) -> isize {
        self.0
    }
}

impl From<isize> for ObservabilityHandle {
    fn from(value: isize) -> Self {
        Self(value)
    }
}

pub mod raw {
    use super::*;
    use crate::{
        data::{AsyncCallback, ByteArray},
        observability::ObservabilityHandle,
        sessions::SessionHandle,
        ProtonSDKLib,
    };

    // int observability_service_start_new(
    //     intptr_t session_handle,
    //     intptr_t* out_observability_handle
    // );
    /// Starts a new observability service
    ///
    /// # Parameters
    /// * `session_handle` - Handle to the active session
    ///
    /// # Returns
    /// (Result code, Observability handle) - code 0 = success, handle for the observability service
    pub fn observability_service_start_new(
        session_handle: SessionHandle,
    ) -> anyhow::Result<(i32, ObservabilityHandle)> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let observability_start_fn: libloading::Symbol<
                unsafe extern "C" fn(isize, *mut isize) -> i32,
            > = sdk.sdk_library.get(b"observability_service_start_new")?;

            let mut observability_handle: isize = 0;
            let result = observability_start_fn(session_handle.raw(), &mut observability_handle);

            Ok((result, ObservabilityHandle::from(observability_handle)))
        }
    }

    // int observability_service_flush(
    //     intptr_t observability_handle,
    //     AsyncCallback callback
    // );
    // int observability_service_start_new(
    //     intptr_t session_handle,
    //     intptr_t* out_observability_handle
    // );
    /// Flushes observability data
    ///
    /// # Parameters
    /// * `observability_handle` - Handle to the observability service
    /// * `callback` - Async callback for completion
    ///
    /// # Returns
    /// Result code (0 = success, non-zero = error)
    pub fn observability_service_flush(
        observability_handle: ObservabilityHandle,
        callback: AsyncCallback,
    ) -> anyhow::Result<i32> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let flush_fn: libloading::Symbol<unsafe extern "C" fn(isize, AsyncCallback) -> i32> =
                sdk.sdk_library.get(b"observability_service_flush")?;

            let result = flush_fn(observability_handle.raw(), callback);

            Ok(result)
        }
    }

    // int observability_service_free(intptr_t observability_handle);
    /// Frees observability service resources
    ///
    /// # Parameters
    /// * `observability_handle` - Handle to the observability service to free
    pub fn observability_service_free(
        observability_handle: ObservabilityHandle,
    ) -> anyhow::Result<()> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let free_fn: libloading::Symbol<unsafe extern "C" fn(isize)> =
                sdk.sdk_library.get(b"observability_service_free")?;

            free_fn(observability_handle.raw());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {}
