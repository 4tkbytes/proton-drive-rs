pub struct LoggerProviderHandle(pub isize);

impl LoggerProviderHandle {
    pub fn null() -> Self {
        Self(0)
    }

    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    pub fn raw(&self) -> isize {
        self.0
    }
}

pub mod raw {
    use crate::{data::Callback, logger::LoggerProviderHandle, observability, ProtonSDKLib};

    // int logger_provider_create(
    //     Callback log_callback,
    //     intptr_t* logger_provider_handle
    // );
    pub fn logger_provider_create(
        log_callback: Callback
    ) -> anyhow::Result<(i32, LoggerProviderHandle)> {
        unsafe {
            let sdk = ProtonSDKLib::instance()?;

            let logger_create: libloading::Symbol<unsafe extern "C" fn(
                Callback,
                *mut isize
            ) -> i32> = sdk.sdk_library.get(b"logger_provider_create")?;

            let mut logger_provider_handle: isize = 0;
            let result = logger_create(log_callback, &mut logger_provider_handle);

            Ok((result, LoggerProviderHandle::from(LoggerProviderHandle(logger_provider_handle))))
        }
    }
}