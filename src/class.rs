use std::{ffi::c_int, ptr::NonNull};

use crate::{
    error::Error, ffi, method::Method, object::Object, sig::Signature, vm::encode_modified_utf8,
};

pub struct Class<'vm> {
    object: Object<'vm>,
}

impl<'vm> Class<'vm> {
    pub(crate) fn from_object(object: Object<'vm>) -> Self {
        Self { object }
    }

    /// the underlying object handle
    pub fn as_object(&self) -> &Object<'vm> {
        &self.object
    }

    pub fn method(&self, name: &str, signature: &str) -> Result<Method<'_>, Error> {
        self.lookup(name, signature, false)
    }

    pub fn static_method(&self, name: &str, signature: &str) -> Result<Method<'_>, Error> {
        self.lookup(name, signature, true)
    }

    fn lookup(&self, name: &str, signature: &str, is_static: bool) -> Result<Method<'_>, Error> {
        // parse first so invalid text never reaches C++!!
        let signature = Signature::parse(signature)?;

        if name.is_empty() || name.contains('\0') {
            return Err(Error::new(
                "method lookup",
                "method name must be nonempty and contain no NUL",
            ));
        }

        let name = encode_modified_utf8(name);
        let signature_text = encode_modified_utf8(signature.as_str());

        let raw = unsafe {
            ffi::usagi_class_get_method(
                self.object.raw.as_ptr(),
                name.as_ptr().cast(),
                signature_text.as_ptr().cast(),
                c_int::from(is_static),
            )
        };

        let id = NonNull::new(raw)
            .ok_or_else(|| Error::from_native("method lookup", "failed to find method"))?;

        Ok(Method {
            id,
            signature,
            is_static,
            class: self.object.raw,
            vm: self.object.vm(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{Vm, ffi};

    #[test]
    fn registers_native_test_method() {
        let vm = Vm::new().expect("VM creation should succeed");

        let status = unsafe { ffi::usagi_test_register_add(vm.raw_ptr()) };
        assert_eq!(status, ffi::USAGI_OK);

        let class = vm
            .find_class("usagi/Test")
            .expect("usagi/Test should exist");
        class
            .static_method("add", "(II)I")
            .expect("add(II)I should be found after registration");
    }
}
