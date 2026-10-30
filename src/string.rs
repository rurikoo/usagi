use crate::{error::Error, ffi, object::Object};

pub struct JavaString<'vm> {
    object: Object<'vm>,
}

impl<'vm> JavaString<'vm> {
    pub(crate) fn from_object(object: Object<'vm>) -> Self {
        Self { object }
    }

    pub fn to_string_lossy(&self) -> Result<String, Error> {
        let mut length = 0usize;

        let status =
            unsafe { ffi::usagi_string_utf16_length(self.object.raw.as_ptr(), &mut length) };

        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "string length",
                "failed to get java string length",
            ));
        }

        let mut units = Vec::<u16>::new();

        units
            .try_reserve_exact(length)
            .map_err(|e| Error::new("string conversion", e.to_string()))?;

        units.resize(length, 0);

        let mut actual_length = 0usize;
        let buffer = if units.is_empty() {
            std::ptr::null_mut()
        } else {
            units.as_mut_ptr()
        };

        let status = unsafe {
            ffi::usagi_string_copy_utf16(
                self.object.raw.as_ptr(),
                buffer,
                units.len(),
                &mut actual_length,
            )
        };

        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "string conversion",
                "failed to copy java string",
            ));
        }

        if actual_length != length {
            return Err(Error::new(
                "string conversion",
                "java string length changed during conversion",
            ));
        }

        Ok(String::from_utf16_lossy(&units))
    }
}
