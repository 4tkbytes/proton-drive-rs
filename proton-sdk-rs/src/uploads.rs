use std::ffi::c_void;
use log::{debug, error};
use tokio::sync::oneshot;
use proton_sdk_sys::{
    data::{AsyncCallback, AsyncCallbackWithProgress, ByteArray, Callback},
    drive::DriveClientHandle,
    protobufs::{FileNode, FileUploadRequest, FileUploaderCreationRequest, IntResponse, Revision},
    uploads::{raw, UploaderHandle},
    cancellation::CancellationTokenHandle,
    prost::Message,
    protobufs::ToByteArray,
};
use proton_sdk_sys::protobufs::{FromByteArray, ProgressUpdate};
use crate::downloads::{DownloadError, Downloader, DownloaderBuilder};
use crate::drive::DriveClient;

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("FFI error: {0}")]
    Ffi(#[from] anyhow::Error),
    #[error("Protobuf error: {0}")]
    Protobuf(#[from] proton_sdk_sys::protobufs::ProtoError),
    #[error("Operation failed with code {0}")]
    Failure(i32),
    #[error("Callback channel closed")]
    CallbackClosed,
    #[error("Uploader handle is null")]
    NullHandle,
}

struct UploadState<F: Fn(f32) + Send + 'static> {
    result_sender: oneshot::Sender<Result<FileNode, UploadError>>,
    progress_callback: Option<F>,
}

pub struct Uploader {
    handle: UploaderHandle,
    _client: DriveClientHandle,
    _token: CancellationTokenHandle,
}

impl Uploader {
    pub async fn new(
        client: DriveClientHandle,
        request: FileUploaderCreationRequest,
        token: CancellationTokenHandle,
    ) -> Result<Self, UploadError> {
        let proto_buf = request.to_proto_buffer()?;
        let (tx, rx) = oneshot::channel::<Result<UploaderHandle, UploadError>>();
        let tx = Box::new(tx);
        let tx_ptr = Box::into_raw(tx);

        extern "C" fn success_callback(state: *const c_void, response: ByteArray) {
            if !state.is_null() {
                unsafe {
                    let tx_ptr = state as *mut oneshot::Sender<Result<UploaderHandle, UploadError>>;
                    let tx = Box::from_raw(tx_ptr);
                    let response = response.as_slice();
                    let handle = match IntResponse::decode(response) {
                        Ok(val) => UploaderHandle::from(val.value as isize),
                        Err(e) => {
                            let _ = tx.send(Err(UploadError::Protobuf(e.into())));
                            return;
                        }
                    };
                    debug!("Uploader created with handle: {:?}", handle);
                    let _ = tx.send(Ok(handle));
                }
            }
        }

        extern "C" fn failure_callback(state: *const c_void, error_data: ByteArray) {
            if !state.is_null() {
                unsafe {
                    let tx_ptr = state as *mut oneshot::Sender<Result<UploaderHandle, UploadError>>;
                    let tx = Box::from_raw(tx_ptr);
                    let error_msg = String::from_utf8_lossy(error_data.as_slice()).to_string();
                    error!("Uploader creation failed: {}", error_msg);
                    let _ = tx.send(Err(UploadError::Ffi(anyhow::anyhow!(error_msg))));
                }
            }
        }

        let async_callback = AsyncCallback::new(
            tx_ptr as *const c_void,
            Some(success_callback),
            Some(failure_callback),
            0, // No cancellation token for now
        );

        let code = raw::uploader_create(client, proto_buf.as_byte_array(), async_callback)?;
        if code != 0 {
            unsafe { let _ = Box::from_raw(tx_ptr); }
            return Err(UploadError::Failure(code));
        }

        let handle = rx.await.map_err(|_| UploadError::CallbackClosed)??;
        if handle.is_null() {
            return Err(UploadError::NullHandle);
        }
        Ok(Uploader { handle, _client: client, _token: token })
    }

    pub async fn upload_file_or_revision<F>(
        &self,
        request: FileUploadRequest,
        progress_callback: Option<F>,
    ) -> Result<FileNode, UploadError>
    where
        F: Fn(f32) + Send + 'static,
    {
        let is_progress_callback = progress_callback.is_some();

        let proto_buf = request.to_proto_buffer()?;
        let (tx, rx) = oneshot::channel::<Result<FileNode, UploadError>>();

        let state = Box::new(UploadState {
            result_sender: tx,
            progress_callback,
        });
        let state_ptr = Box::into_raw(state);

        extern "C" fn success_callback<F: Fn(f32) + Send + 'static>(
            state: *const c_void,
            response: ByteArray,
        ) {
            if !state.is_null() {
                unsafe {
                    let state_ptr = state as *mut UploadState<F>;
                    let state = Box::from_raw(state_ptr);
                    let response = response.as_slice();
                    let node = match FileNode::decode(response) {
                        Ok(val) => Ok(val),
                        Err(e) => Err(UploadError::Protobuf(e.into())),
                    };
                    let _ = state.result_sender.send(node);
                }
            }
        }

        extern "C" fn failure_callback<F: Fn(f32) + Send + 'static>(
            state: *const c_void,
            error_data: ByteArray,
        ) {
            if !state.is_null() {
                unsafe {
                    let state_ptr = state as *mut UploadState<F>;
                    let state = Box::from_raw(state_ptr);
                    let error_msg = String::from_utf8_lossy(error_data.as_slice()).to_string();
                    let _ = state.result_sender.send(Err(UploadError::Ffi(anyhow::anyhow!(error_msg))));
                }
            }
        }

        let async_callback = AsyncCallback::new(
            state_ptr as *const c_void,
            Some(success_callback::<F>),
            Some(failure_callback::<F>),
            self._token.raw(),
        );
        let progress_cb = if is_progress_callback {
            Callback::new(state_ptr as *const c_void, Some(progress_callback_fn::<F>))
        } else {
            Callback::empty()
        };
        let async_callback_with_progress = AsyncCallbackWithProgress::new(async_callback, progress_cb);

        let code = raw::uploader_upload_file_or_revision(self.handle, proto_buf.as_byte_array(), async_callback_with_progress)?;
        if code != 0 {
            unsafe { let _ = Box::from_raw(state_ptr); }
            return Err(UploadError::Failure(code));
        }

        rx.await.map_err(|_| UploadError::CallbackClosed)?
    }

    pub async fn upload_revision<F>(
        &self,
        request: FileUploadRequest,
        progress_callback: Option<F>,
    ) -> Result<Revision, UploadError>
    where
        F: Fn(f32) + Send + 'static,
    {
        let is_progress_callback = progress_callback.is_some();
        let proto_buf = request.to_proto_buffer()?;
        let (tx, rx) = oneshot::channel::<Result<Revision, UploadError>>();

        struct UploadState<F: Fn(f32) + Send + 'static> {
            result_sender: oneshot::Sender<Result<Revision, UploadError>>,
            progress_callback: Option<F>,
        }

        let state = Box::new(UploadState {
            result_sender: tx,
            progress_callback,
        });
        let state_ptr = Box::into_raw(state);

        extern "C" fn success_callback<F: Fn(f32) + Send + 'static>(
            state: *const c_void,
            response: ByteArray,
        ) {
            if !state.is_null() {
                unsafe {
                    let state_ptr = state as *mut UploadState<F>;
                    let state = Box::from_raw(state_ptr);
                    let response = response.as_slice();
                    let rev = match Revision::decode(response) {
                        Ok(val) => Ok(val),
                        Err(e) => Err(UploadError::Protobuf(e.into())),
                    };
                    let _ = state.result_sender.send(rev);
                }
            }
        }

        extern "C" fn failure_callback<F: Fn(f32) + Send + 'static>(
            state: *const c_void,
            error_data: ByteArray,
        ) {
            if !state.is_null() {
                unsafe {
                    let state_ptr = state as *mut UploadState<F>;
                    let state = Box::from_raw(state_ptr);
                    let error_msg = String::from_utf8_lossy(error_data.as_slice()).to_string();
                    let _ = state.result_sender.send(Err(UploadError::Ffi(anyhow::anyhow!(error_msg))));
                }
            }
        }

        let async_callback = AsyncCallback::new(
            state_ptr as *const c_void,
            Some(success_callback::<F>),
            Some(failure_callback::<F>),
            self._token.raw(),
        );
        let progress_cb = if is_progress_callback {
            Callback::new(state_ptr as *const c_void, Some(progress_callback_fn::<F>))
        } else {
            Callback::empty()
        };
        let async_callback_with_progress = AsyncCallbackWithProgress::new(async_callback, progress_cb);

        let code = raw::uploader_upload_revision(self.handle, proto_buf.as_byte_array(), async_callback_with_progress)?;
        if code != 0 {
            unsafe { let _ = Box::from_raw(state_ptr); }
            return Err(UploadError::Failure(code));
        }

        rx.await.map_err(|_| UploadError::CallbackClosed)?
    }
}

extern "C" fn progress_callback_fn<F: Fn(f32) + Send + 'static>(
    state: *const c_void,
    progress_data: ByteArray,
) {
    if !state.is_null() {
        unsafe {
            let state_ptr = state as *const UploadState<F>;
            let state = &*state_ptr;
            let bytes = progress_data.as_slice();
            let progress = ProgressUpdate::from_bytes(bytes).expect("No progress update data");
            if let Some(ref callback) = state.progress_callback {
                // completed out of total as percent
                callback((progress.bytes_completed / progress.bytes_in_total) as f32);
            }
        }
    }
}

impl Drop for Uploader {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            if let Err(e) = raw::uploader_free(self.handle) {
                error!("Failed to free uploader in Drop: {}", e);
            } else {
                debug!("Uploader cleaned up automatically");
            }
        }
    }
}

pub struct UploaderBuilder {
    client: DriveClientHandle,
    request: FileUploaderCreationRequest,
    token: CancellationTokenHandle
}

impl UploaderBuilder {
    pub fn new(client: &DriveClient) -> Self {
        Self {
            client: client.handle(), 
            request: FileUploaderCreationRequest::default(), 
            token: client.session().cancellation_token().handle() 
        }
    }
    
    pub fn with_request(self, request: FileUploaderCreationRequest) -> Self {
        Self { request, ..self }
    }

    pub async fn build(
        self
    ) -> Result<Uploader, UploadError> {
        Uploader::new(self.client, self.request, self.token).await
    }
}