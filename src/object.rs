use std::ptr::NonNull;

use crate::{error::Error, ffi, vm::Vm};

pub struct Object<'vm> {
    pub(crate) raw: NonNull<ffi::UsagiObject>,
    vm: &'vm Vm,
}

impl<'vm> Object<'vm> {
    pub(crate) fn from_raw(
        vm: &'vm Vm,
        raw: *mut ffi::UsagiObject,
        operation: &'static str,
        fallback: &str,
    ) -> Result<Self, Error> {
        let raw = NonNull::new(raw).ok_or_else(|| Error::from_native(operation, fallback))?;

        Ok(Self { raw, vm })
    }

    #[allow(dead_code)]
    /// true when this object was created by vm
    pub(crate) fn belongs_to(&self, vm: &Vm) -> bool {
        std::ptr::eq(self.vm, vm)
    }
}

impl Drop for Object<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::usagi_object_destroy(self.raw.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Vm;

    #[test]
    fn objects_belong_only_to_their_vm() {
        let a = Vm::new().unwrap();
        let b = Vm::new().unwrap();

        let string = a.new_string("hi").unwrap();

        assert!(string.object().belongs_to(&a));
        assert!(!string.object().belongs_to(&b));
    }
}
