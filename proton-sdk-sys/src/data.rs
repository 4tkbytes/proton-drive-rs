use std::os::raw::c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ByteArray {
    pub pointer: *const u8,
    pub length: usize,
}

impl ByteArray {
    pub fn from_slice(data: &[u8]) -> Self {
        Self {
            pointer: data.as_ptr(),
            length: data.len(),
        }
    }

    /// Create an empty ByteArray
    pub fn empty() -> Self {
        Self {
            pointer: std::ptr::null(),
            length: 0,
        }
    }

    /// Convert ByteArray to a Rust slice (unsafe)
    pub unsafe fn as_slice(&self) -> &[u8] {
        if self.pointer.is_null() || self.length == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(self.pointer, self.length)
        }
    }

    /// Check if the ByteArray is empty
    pub fn is_empty(&self) -> bool {
        self.length == 0 || self.pointer.is_null()
    }
}

#[repr(C)]
pub struct AsyncCallback {
    pub state: *const c_void,
    pub on_success: Option<extern "C" fn(*const c_void, ByteArray)>,
    pub on_failure: Option<extern "C" fn(*const c_void, ByteArray)>,
    pub cancellation_token_source_handle: isize,
}

impl AsyncCallback {
    /// Creates a new AsyncCallback instance
    pub fn new(
        state: *const c_void,
        on_success: Option<extern "C" fn(*const c_void, ByteArray)>,
        on_failure: Option<extern "C" fn(*const c_void, ByteArray)>,
        cancellation_token_source_handle: isize,
    ) -> Self {
        Self {
            state,
            on_success,
            on_failure,
            cancellation_token_source_handle,
        }
    }

    /// Create an AsyncCallback with null callbacks
    pub fn empty(cancellation_token_source_handle: isize) -> Self {
        Self {
            state: std::ptr::null(),
            on_success: None,
            on_failure: None,
            cancellation_token_source_handle,
        }
    }
}

#[repr(C)]
pub struct Callback {
    state: *const c_void,
    callback: Option<extern "C" fn(*const c_void, ByteArray)>,
}

impl Callback {
    /// New instance of callback
    pub fn new(
        state: *const c_void,
        callback: Option<extern "C" fn(*const c_void, ByteArray)>,
    ) -> Self {
        Self { state, callback }
    }

    /// Empty instance of callback
    pub fn empty() -> Self {
        Self {
            state: std::ptr::null(),
            callback: None,
        }
    }
}

#[repr(C)]
pub struct AsyncCallbackWithProgress {
    pub async_callback: AsyncCallback,
    pub progress_callback: Callback,
}

impl AsyncCallbackWithProgress {
    /// Create a new AsyncCallbackWithProgress
    pub fn new(async_callback: AsyncCallback, progress_callback: Callback) -> Self {
        Self {
            async_callback,
            progress_callback,
        }
    }

    /// Create an AsyncCallbackWithProgress with empty callbacks
    pub fn empty(cancellation_token_source_handle: isize) -> Self {
        Self {
            async_callback: AsyncCallback::empty(cancellation_token_source_handle),
            progress_callback: Callback::empty(),
        }
    }
}

#[repr(C)]
pub struct BooleanCallback {
    pub state: *const c_void,
    pub callback: Option<extern "C" fn(*const c_void, ByteArray) -> bool>,
}

impl BooleanCallback {
    /// Create a new BooleanCallback
    pub fn new(
        state: *const c_void,
        callback: Option<extern "C" fn(*const c_void, ByteArray) -> bool>,
    ) -> Self {
        Self { state, callback }
    }

    /// Create an empty BooleanCallback
    pub fn empty() -> Self {
        Self {
            state: std::ptr::null(),
            callback: None,
        }
    }
}

#[repr(C)]
/// The callback looks confusing, but it really is not:
/// 
/// # Callback Params
/// * The first `ByteArray` is for a 2 factor auth code.
/// * The second `ByteArray` is for an optional data password.
/// It's recommended to set the optional data password to the users password
/// if they do not have one
/// 
/// Note: The `ByteArray` of the out params are both `StringResponse` types,
/// which contain a value of a string. 
pub struct TwoFactorRequestedCallback {
    pub state: *const c_void,
    pub callback: Option<extern "C" fn(*const c_void, ByteArray, *mut ByteArray, *mut ByteArray) -> bool>,
}

impl TwoFactorRequestedCallback {
    /// Create a new TwoFactorRequestedCallback
    pub fn new(
        state: *const c_void,
        callback: Option<extern "C" fn(*const c_void, ByteArray, *mut ByteArray, *mut ByteArray) -> bool>,
    ) -> Self {
        Self { state, callback }
    }

    /// Create an empty TwoFactorRequestedCallback
    pub fn empty() -> Self {
        Self {
            state: std::ptr::null(),
            callback: None,
        }
    }
}