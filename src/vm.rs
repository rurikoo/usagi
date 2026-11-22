use std::{ffi::CString, ptr::NonNull, sync::Arc};

use crate::{
    RawJavaVM,
    env::{AttachGuard, Env},
    error::Error,
    ffi,
};

pub(crate) struct VmInner {
    pub(crate) raw: NonNull<ffi::UsagiVm>,
}

unsafe impl Send for VmInner {}
unsafe impl Sync for VmInner {}

impl Drop for VmInner {
    fn drop(&mut self) {
        unsafe {
            ffi::usagi_vm_destroy(self.raw.as_ptr());
        }
    }
}

pub struct Vm {
    inner: Arc<VmInner>,
}

impl Vm {
    pub fn new() -> Result<Self, Error> {
        let raw = unsafe { ffi::usagi_vm_create() };

        let raw = NonNull::new(raw)
            .ok_or_else(|| Error::from_native("VM creation", "failed to create JNI VM"))?;

        Ok(Self {
            inner: Arc::new(VmInner { raw }),
        })
    }

    pub fn get_java_vm(&self) -> Result<NonNull<RawJavaVM>, Error> {
        let raw = unsafe { ffi::usagi_vm_get_java_vm(self.inner.raw.as_ptr()) };

        NonNull::new(raw).ok_or_else(|| Error::from_native("JavaVM access", "JavaVM unavailable"))
    }

    /// returns a JNI environment for an already-attached thread
    pub fn env(&self) -> Result<Env<'_>, Error> {
        let raw = unsafe { ffi::usagi_vm_get_jni_env(self.inner.raw.as_ptr()) };

        let raw_env = NonNull::new(raw)
            .ok_or_else(|| Error::from_native("JNIEnv access", "JNIEnv unavailable"))?;

        Ok(Env::new(self, raw_env))
    }

    /// attaches the current thread (if not already attached) and returns an RAII guard
    pub fn attach_current_thread(&self) -> Result<AttachGuard<'_>, Error> {
        let mut raw_env = std::ptr::null_mut();
        let mut attached = 0;

        let status = unsafe {
            ffi::usagi_vm_attach_current_thread(
                self.inner.raw.as_ptr(),
                &mut raw_env,
                &mut attached,
            )
        };

        if status != ffi::JNI_OK {
            return Err(Error::from_native(
                "thread attachment",
                "failed to attach current thread",
            ));
        }

        let raw_env = NonNull::new(raw_env).ok_or_else(|| {
            Error::new(
                "thread attachment",
                "attachment succeeded but returned a null JNIEnv",
            )
        })?;

        Ok(AttachGuard::new(Env::new(self, raw_env), attached != 0))
    }

    pub fn generate_class_dump(&self, path: &str) -> Result<(), Error> {
        let path = CString::new(path)
            .map_err(|_| Error::new("class dump", "path contains an embedded NUL"))?;

        let status =
            unsafe { ffi::usagi_vm_generate_class_dump(self.inner.raw.as_ptr(), path.as_ptr()) };

        if status != ffi::JNI_OK {
            return Err(Error::from_native(
                "class dump",
                "failed to generate class dump",
            ));
        }

        Ok(())
    }

    pub(crate) fn inner(&self) -> &Arc<VmInner> {
        &self.inner
    }

    pub(crate) fn raw_ptr(&self) -> *mut ffi::UsagiVm {
        self.inner.raw.as_ptr()
    }

    pub(crate) fn same_vm(&self, other: &Arc<VmInner>) -> bool {
        Arc::ptr_eq(&self.inner, other)
    }
}

pub(crate) fn encode_modified_utf8(value: &str) -> Vec<u8> {
    let mut output = Vec::new();

    for unit in value.encode_utf16() {
        match unit {
            0x0001..=0x007F => output.push(unit as u8),
            0x0000..=0x07FF => {
                output.push((0xC0 | (unit >> 6)) as u8);
                output.push((0x80 | (unit & 0x3F)) as u8);
            }
            _ => {
                output.push((0xE0 | (unit >> 12)) as u8);
                output.push((0x80 | ((unit >> 6) & 0x3F)) as u8);
                output.push((0x80 | (unit & 0x3F)) as u8);
            }
        }
    }

    output.push(0);
    output
}
