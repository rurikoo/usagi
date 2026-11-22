use std::{marker::PhantomData, ops::Deref, ptr::NonNull, rc::Rc, sync::Arc};

use crate::{
    RawJNIEnv,
    class::Class,
    error::Error,
    ffi,
    object::{Object, ObjectKind},
    string::JavaString,
    vm::{Vm, encode_modified_utf8},
};

/// a JNI environment handle
pub struct Env<'vm> {
    vm: &'vm Vm,
    raw: NonNull<RawJNIEnv>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl<'vm> Env<'vm> {
    pub(crate) fn new(vm: &'vm Vm, raw: NonNull<RawJNIEnv>) -> Self {
        Self {
            vm,
            raw,
            _not_send_sync: PhantomData,
        }
    }

    pub fn raw_jni_env(&self) -> NonNull<RawJNIEnv> {
        self.raw
    }

    pub fn vm(&self) -> &'vm Vm {
        self.vm
    }

    /// defines a class, libjnivm creates unknown classes on demand
    pub fn define_class(&self, internal_name: &str) -> Result<Class<'_>, Error> {
        self.find_class(internal_name)
    }

    /// looks up a class using its JVM internal name, for example "java/lang/String"
    pub fn find_class(&self, internal_name: &str) -> Result<Class<'_>, Error> {
        if internal_name.is_empty() || internal_name.contains('\0') {
            return Err(Error::new(
                "class lookup",
                "class name must be nonempty and contain no NUL",
            ));
        }

        let name = encode_modified_utf8(internal_name);
        let raw = unsafe { ffi::usagi_vm_find_class(self.vm.raw_ptr(), name.as_ptr().cast()) };

        let object = Object::from_raw(
            Arc::clone(self.vm.inner()),
            raw,
            ObjectKind::Class,
            "class lookup",
            "failed to find class",
        )?;

        Ok(Class::from_object(object))
    }

    /// creates a java string from a rust UTF-8 string
    pub fn new_string(&self, value: &str) -> Result<JavaString<'_>, Error> {
        let units: Vec<u16> = value.encode_utf16().collect();

        if units.len() > i32::MAX as usize {
            return Err(Error::new(
                "string creation",
                "string exceeds the maximum JNI length",
            ));
        }

        let units_ptr = if units.is_empty() {
            std::ptr::null()
        } else {
            units.as_ptr()
        };

        let raw =
            unsafe { ffi::usagi_vm_new_string_utf16(self.vm.raw_ptr(), units_ptr, units.len()) };

        let object = Object::from_raw(
            Arc::clone(self.vm.inner()),
            raw,
            ObjectKind::String,
            "string creation",
            "failed to create Java string",
        )?;

        Ok(JavaString::from_object(object))
    }
}

/// guard for a thread attached via Vm::attach_current_thread.
pub struct AttachGuard<'vm> {
    env: Env<'vm>,
    attached: bool,
}

impl<'vm> AttachGuard<'vm> {
    pub(crate) fn new(env: Env<'vm>, attached: bool) -> Self {
        Self { env, attached }
    }

    pub fn env(&self) -> &Env<'vm> {
        &self.env
    }
}

impl<'vm> Deref for AttachGuard<'vm> {
    type Target = Env<'vm>;

    fn deref(&self) -> &Self::Target {
        &self.env
    }
}

impl Drop for AttachGuard<'_> {
    fn drop(&mut self) {
        if self.attached {
            let _ = unsafe { ffi::usagi_vm_detach_current_thread(self.env.vm.raw_ptr()) };
        }
    }
}
