use crate::data::ByteArray;
use prost::Message;

// Include the generated protobuf code
include!(concat!(env!("OUT_DIR"), "/_.rs"));

#[derive(Debug, thiserror::Error)]
pub enum ProtoError {
    #[error("Failed to encode protobuf message: {0}")]
    EncodeError(#[from] prost::EncodeError),

    #[error("Failed to decode protobuf message: {0}")]
    DecodeError(#[from] prost::DecodeError),

    #[error("ByteArray contains invalid data")]
    InvalidData,
}

pub struct ProtoBuffer {
    /// The encoded bytes - must stay alive while ByteArray is used
    _buffer: Vec<u8>,
    /// The ByteArray pointing to the buffer data
    byte_array: ByteArray,
}

impl ProtoBuffer {
    /// Encodes a protobuf message into a ProtoBuffer
    ///
    /// # Example
    /// ```rust
    /// use crate::protobufs::SessionBeginRequest;
    ///
    /// let request = SessionBeginRequest {
    ///     username: "user@example.com".to_string(),
    ///     password: "password".to_string(),
    ///     options: None,
    /// };
    ///
    /// let proto_buf = ProtoBuffer::encode(&request)?;
    /// let result = raw::session_begin(0, proto_buf.as_byte_array(), callbacks...)?;
    /// ```
    pub fn encode<T: Message>(message: &T) -> Result<Self, ProtoError> {
        let mut buffer = Vec::new();
        message.encode(&mut buffer)?;

        let byte_array = ByteArray::from_slice(&buffer);

        Ok(Self {
            _buffer: buffer,
            byte_array,
        })
    }

    /// Gets the ByteArray for FFI calls
    pub fn as_byte_array(&self) -> ByteArray {
        self.byte_array
    }

    /// Gets the raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self._buffer
    }

    /// Gets the size of the encoded data
    pub fn len(&self) -> usize {
        self._buffer.len()
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self._buffer.is_empty()
    }
}

/// Helper trait for encoding protobuf messages to ByteArray
pub trait ToByteArray {
    /// Encodes the message and returns a ProtoBuffer that manages the lifetime
    fn to_proto_buffer(&self) -> Result<ProtoBuffer, ProtoError>;

    /// Encodes the message to a Vec<u8>
    fn to_bytes(&self) -> Result<Vec<u8>, ProtoError>;
}

/// Helper trait for decoding protobuf messages from ByteArray
pub trait FromByteArray: Sized {
    /// Decodes a message from a ByteArray
    fn from_byte_array(data: &ByteArray) -> Result<Self, ProtoError>;

    /// Decodes a message from raw bytes
    fn from_bytes(data: &[u8]) -> Result<Self, ProtoError>;
}

/// Implement ToByteArray for all protobuf messages
impl<T: Message> ToByteArray for T {
    fn to_proto_buffer(&self) -> Result<ProtoBuffer, ProtoError> {
        ProtoBuffer::encode(self)
    }

    fn to_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        let mut buffer = Vec::new();
        self.encode(&mut buffer)?;
        Ok(buffer)
    }
}

/// Implement FromByteArray for all protobuf messages
impl<T: Message + Default> FromByteArray for T {
    fn from_byte_array(data: &ByteArray) -> Result<Self, ProtoError> {
        unsafe {
            let slice = data.as_slice();
            Ok(T::decode(slice)?)
        }
    }

    fn from_bytes(data: &[u8]) -> Result<Self, ProtoError> {
        Ok(T::decode(data)?)
    }
}

/// Convenience functions for common protobuf operations
pub mod helpers {
    use super::*;

    /// Encodes a protobuf message and returns (buffer, ByteArray) tuple
    /// The buffer must be kept alive while using the ByteArray
    pub fn encode_message<T: Message>(message: &T) -> Result<(Vec<u8>, ByteArray), ProtoError> {
        let mut buffer = Vec::new();
        message.encode(&mut buffer)?;
        let byte_array = ByteArray::from_slice(&buffer);
        Ok((buffer, byte_array))
    }

    /// Decodes a protobuf message from a ByteArray
    pub fn decode_message<T: Message + Default>(data: &ByteArray) -> Result<T, ProtoError> {
        T::from_byte_array(data)
    }

    /// Decodes a protobuf message from raw bytes
    pub fn decode_bytes<T: Message + Default>(data: &[u8]) -> Result<T, ProtoError> {
        T::from_bytes(data)
    }

    /// Creates an empty ByteArray (useful for testing)
    pub fn empty_byte_array() -> ByteArray {
        ByteArray::from_slice(&[])
    }
}

pub mod callbacks {
    use super::{FromByteArray, ProtoError};
    use crate::data::ByteArray;
    use prost::Message;
    use std::ffi::c_void;

    /// Helper for handling protobuf responses in success callbacks
    ///
    /// # Example
    /// ```rust
    /// use proton_sdk_sys::data::ByteArray;
    /// use std::ffi::c_void;
    /// use proton_sdk_sys::protobufs::callbacks::handle_protobuf_response;
    /// use proton_sdk_sys::protobufs::SessionTokens;
    ///
    /// extern "C" fn session_success_callback(_state: *const c_void, response: ByteArray) {
    ///     handle_protobuf_response(&response, |tokens: SessionTokens| {
    ///         println!("Received tokens: access_token len = {}", tokens.access_token.len());
    ///     });
    /// }
    /// ```
    pub fn handle_protobuf_response<T, F>(data: &ByteArray, handler: F)
    where
        T: Message + Default,
        F: FnOnce(T),
    {
        match T::from_byte_array(data) {
            Ok(message) => handler(message),
            Err(e) => eprintln!("Failed to decode protobuf response: {}", e),
        }
    }

    /// Helper for handling protobuf errors in failure callbacks
    pub fn handle_protobuf_error(data: &ByteArray) -> Option<super::Error> {
        super::Error::from_byte_array(data).ok()
    }

    /// Generic callback wrapper that decodes protobuf and calls user function
    pub struct ProtobufCallback<T: Message + Default> {
        callback: Box<dyn Fn(T) + Send + Sync>,
    }

    impl<T: Message + Default> ProtobufCallback<T> {
        pub fn new<F>(callback: F) -> Self
        where
            F: Fn(T) + Send + Sync + 'static,
        {
            Self {
                callback: Box::new(callback),
            }
        }

        /// Returns the C callback function pointer
        pub extern "C" fn c_callback(state: *const c_void, data: ByteArray) {
            if !state.is_null() {
                unsafe {
                    let wrapper = &*(state as *const ProtobufCallback<T>);
                    if let Ok(message) = T::from_byte_array(&data) {
                        (wrapper.callback)(message);
                    }
                }
            }
        }
    }
}

/// Validation helpers for protobuf messages
pub mod validation {
    use super::*;

    /// Validates that required fields are present
    pub trait Validate {
        type Error;
        fn validate(&self) -> Result<(), Self::Error>;
    }

    // Example validation for SessionBeginRequest
    // impl Validate for SessionBeginRequest {
    //     type Error = &'static str;

    //     fn validate(&self) -> Result<(), Self::Error> {
    //         if self.username.is_empty() {
    //             return Err("Username is required");
    //         }
    //         if self.password.is_empty() {
    //             return Err("Password is required");
    //         }
    //         Ok(())
    //     }
    // }
}
