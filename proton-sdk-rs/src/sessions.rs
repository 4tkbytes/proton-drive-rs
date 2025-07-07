#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("SDK error: {0}")]
    SdkError(#[from] anyhow::Error),

    #[error("Session operation failed")]
    OperationFailed(i32),

    #[error("Protobuf error: {0}")]
    ProtobufError(#[from] proton_sdk_sys::protobufs::ProtoError),

    #[error("Session handle is null")]
    NullHandle,
    
    #[error("Operation was cancelled")]
    Cancelled,
}