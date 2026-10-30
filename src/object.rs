use std::{marker::PhantomData, ptr::NonNull};

use crate::{error::Error, ffi, vm::Vm};

pub struct Object<'vm> {
    pub(crate) raw: NonNull<ffi::UsagiObject>,
    _vm: PhantomData<&'vm Vm>,
}

impl<'vm> Object<'vm> {
    pub(crate) fn from_raw(
        _vm: &'vm Vm,
        raw: *mut ffi::UsagiObject,
        operation: &'static str,
        fallback: &str,
    ) -> Result<Self, Error> {
        let raw = NonNull::new(raw).ok_or_else(|| Error::from_native(operation, fallback))?;

        Ok(Self {
            raw,
            _vm: PhantomData,
        })
    }
}

impl Drop for Object<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::usagi_object_destroy(self.raw.as_ptr());
        }
    }
}
