use proton_sdk_sys::cancellation::{raw, CancellationTokenHandle};

// Todo
pub struct CancellationTokenSource {
    handle: CancellationTokenHandle,
}

impl CancellationTokenSource {
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
        // Prevent Drop from running
        std::mem::forget(self);
        result
    }
}

impl Drop for CancellationTokenSource {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // Ignore errors in Drop - we can't handle them anyway
            let _ = raw::free(self.handle.raw());
        }
    }
}

// Implement common traits for ergonomic usage
impl Default for CancellationTokenSource {
    fn default() -> Self {
        Self::new().expect("Failed to create cancellation token")
    }
}

#[cfg(test)]
mod tests {
}