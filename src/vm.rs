use std::{ffi::CString, marker::PhantomData, ptr::NonNull, rc::Rc};

use crate::{RawJNIEnv, RawJavaVM, error::Error, ffi, object::Object, string::JavaString};

pub struct Vm {
    raw: NonNull<ffi::UsagiVm>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl Vm {
    pub fn new() -> Result<Self, Error> {
        let raw = unsafe { ffi::usagi_vm_create() };

        let raw = NonNull::new(raw)
            .ok_or_else(|| Error::from_native("VM creation", "failed to create JNI VM"))?;

        Ok(Self {
            raw,
            _not_send_sync: PhantomData,
        })
    }

    pub fn get_java_vm(&self) -> Result<NonNull<RawJavaVM>, Error> {
        let raw = unsafe { ffi::usagi_vm_get_java_vm(self.raw.as_ptr()) };

        NonNull::new(raw).ok_or_else(|| Error::from_native("JavaVM access", "JavaVM unavailable"))
    }

    pub fn get_jni_env(&self) -> Result<NonNull<RawJNIEnv>, Error> {
        let raw = unsafe { ffi::usagi_vm_get_jni_env(self.raw.as_ptr()) };

        NonNull::new(raw).ok_or_else(|| Error::from_native("JNIEnv access", "JNIEnv unavailable"))
    }

    pub fn attach_current_thread(&self) -> Result<NonNull<RawJNIEnv>, Error> {
        let mut env = std::ptr::null_mut();

        let status = unsafe { ffi::usagi_vm_attach_current_thread(self.raw.as_ptr(), &mut env) };

        if status != ffi::JNI_OK {
            return Err(Error::from_native(
                "thread attachment",
                "failed to attach current thread",
            ));
        }

        NonNull::new(env).ok_or_else(|| {
            Error::new(
                "thread attachment",
                "attachment succeeded but returned a null JNIEnv",
            )
        })
    }

    pub fn detach_current_thread(&self) -> Result<(), Error> {
        let status = unsafe { ffi::usagi_vm_detach_current_thread(self.raw.as_ptr()) };

        if status != ffi::JNI_OK {
            return Err(Error::from_native(
                "thread detachment",
                "failed to detach current thread",
            ));
        }

        Ok(())
    }

    /// looks up a class using its JVM internal name, for example "java/lang/String"
    pub fn find_class(&self, internal_name: &str) -> Result<Object<'_>, Error> {
        if internal_name.is_empty() || internal_name.contains('\0') {
            return Err(Error::new(
                "class lookup",
                "class name must be nonempty and contain no NUL",
            ));
        }

        let name = encode_modified_utf8(internal_name);

        let raw = unsafe { ffi::usagi_vm_find_class(self.raw.as_ptr(), name.as_ptr().cast()) };

        Object::from_raw(self, raw, "class lookup", "failed to find class")
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
            unsafe { ffi::usagi_vm_new_string_utf16(self.raw.as_ptr(), units_ptr, units.len()) };

        let object =
            Object::from_raw(self, raw, "string creation", "failed to create Java string")?;

        Ok(JavaString::from_object(object))
    }

    pub fn generate_class_dump(&self, path: &str) -> Result<(), Error> {
        let path = CString::new(path)
            .map_err(|_| Error::new("class dump", "path contains an embedded NUL"))?;

        let status = unsafe { ffi::usagi_vm_generate_class_dump(self.raw.as_ptr(), path.as_ptr()) };

        if status != ffi::JNI_OK {
            return Err(Error::from_native(
                "class dump",
                "failed to generate class dump",
            ));
        }

        Ok(())
    }
}

impl Drop for Vm {
    fn drop(&mut self) {
        unsafe {
            ffi::usagi_vm_destroy(self.raw.as_ptr());
        }
    }
}

fn encode_modified_utf8(value: &str) -> Vec<u8> {
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
