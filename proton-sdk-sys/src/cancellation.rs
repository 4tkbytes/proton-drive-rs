use crate::{cancellation, ProtonSDKLib};

/// Handle for a cancellation token source (raw type)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancellationTokenHandle(pub isize);

impl CancellationTokenHandle {
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

    /// Checks if the handle is "None"
    pub fn is_none(&self) -> bool {
        self.0 == -1
    }
}

impl From<isize> for CancellationTokenHandle {
    fn from(handle: isize) -> Self {
        Self(handle)
    }
}

pub mod raw {
    use super::*;

    /// Creates a cancellation token source (raw FFI)
    pub fn create() -> anyhow::Result<isize> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let create_fn: libloading::Symbol<unsafe extern "C" fn() -> isize> =
                sdk.sdk_library.get(b"cancellation_token_source_create")?;

            let handle = create_fn();

            if handle == 0 {
                return Err(anyhow::anyhow!(
                    "Failed to create cancellation token source"
                ));
            }

            Ok(handle)
        }
    }

    /// Cancels a cancellation token source (raw FFI)
    /// Note: Does nothing if handle is CancellationToken::NONE
    pub fn cancel(handle: isize) -> anyhow::Result<()> {
        // Don't try to cancel the "None" token
        if handle == -1 {
            return Ok(());
        }

        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let cancel_fn: libloading::Symbol<unsafe extern "C" fn(isize)> =
                sdk.sdk_library.get(b"cancellation_token_source_cancel")?;

            cancel_fn(handle);
            Ok(())
        }
    }

    /// Frees a cancellation token source (raw FFI)
    /// Note: Does nothing if handle is CancellationToken::NONE
    pub fn free(handle: isize) -> anyhow::Result<()> {
        // Don't try to free the "None" token
        if handle == -1 {
            return Ok(());
        }

        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let free_fn: libloading::Symbol<unsafe extern "C" fn(isize)> =
                sdk.sdk_library.get(b"cancellation_token_source_free")?;

            free_fn(handle);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_cancellation_functions() {
        if let Ok(handle) = raw::create() {
            assert_ne!(handle, 0);
            assert!(raw::cancel(handle).is_ok());
            assert!(raw::free(handle).is_ok());
            println!("✓ Raw cancellation functions work");
        }
    }
}
