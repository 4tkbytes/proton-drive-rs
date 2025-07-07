use proton_sdk_sys::cancellation::{raw, CancellationTokenHandle};

// Todo
pub struct CancellationToken {
    handle: CancellationTokenHandle,
}

impl CancellationToken {
    /// Creates a new cancellation token source
    pub fn new() -> anyhow::Result<Self> {
        let handle = raw::create()?;
        Ok(Self {
            handle: CancellationTokenHandle(handle),
        })
    }

    /// Fetches the handle
    pub fn handle(&self) -> CancellationTokenHandle {
        self.handle
    }

    /// Cancels all operations associated with this token
    pub fn cancel(&self) -> anyhow::Result<()> {
        raw::cancel(self.handle.raw())
    }

    /// Free the cancellation token source
    pub fn free(self) -> anyhow::Result<()> {
        let result = raw::free(self.handle.raw());
        std::mem::forget(self);
        result
    }
}

impl Drop for CancellationToken {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            let _ = raw::free(self.handle.raw());
        }
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new().expect("Failed to create cancellation token")
    }
}

impl Clone for CancellationToken {
    fn clone(&self) -> Self {
        // not ideal but safe
        Self::new().unwrap_or_else(|_| Self {
            handle: CancellationTokenHandle::null(),
        })
    }
}

#[cfg(test)]
mod tests {}
