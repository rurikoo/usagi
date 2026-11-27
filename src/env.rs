use std::{ffi::c_void, marker::PhantomData, ops::Deref, ptr::NonNull, rc::Rc, sync::Arc};

use crate::{
    RawJNIEnv,
    array::{ArrayElement, JavaArray},
    class::Class,
    error::Error,
    ffi,
    object::Object,
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
            "string creation",
            "failed to create java string",
        )?;

        Ok(JavaString::from_object(object))
    }

    /// creates a zero-initialized primitive java array of the requested element type
    pub fn new_primitive_array<T: ArrayElement>(
        &self,
        length: usize,
    ) -> Result<JavaArray<'_>, Error> {
        let raw = unsafe {
            ffi::usagi_vm_new_primitive_array(self.vm.raw_ptr(), T::KIND.to_ffi(), length)
        };
        let object = Object::from_raw(
            Arc::clone(self.vm.inner()),
            raw,
            "array creation",
            "failed to create java primitive array",
        )?;
        Ok(JavaArray::from_object(object))
    }

    /// creates a primitive java array initialized from a Rust slice
    pub fn new_primitive_array_from_slice<T: ArrayElement>(
        &self,
        slice: &[T],
    ) -> Result<JavaArray<'_>, Error> {
        let array = self.new_primitive_array::<T>(slice.len())?;
        array.set_region(0, slice)?;
        Ok(array)
    }

    /// creates a java object array of element_class with optional initial element
    pub fn new_object_array(
        &self,
        length: usize,
        element_class: &Class<'_>,
        init: Option<&Object<'_>>,
    ) -> Result<JavaArray<'_>, Error> {
        if !element_class.as_object().belongs_to_inner(self.vm.inner()) {
            return Err(Error::new(
                "array creation",
                "element class belongs to a different vm",
            ));
        }
        if let Some(obj) = init {
            if !obj.belongs_to_inner(self.vm.inner()) {
                return Err(Error::new(
                    "array creation",
                    "initial element belongs to a different vm",
                ));
            }
        }

        let init_ptr = init.map_or(std::ptr::null(), |o| o.raw().as_ptr().cast_const());
        let raw = unsafe {
            ffi::usagi_vm_new_object_array(
                self.vm.raw_ptr(),
                length,
                element_class.as_object().raw().as_ptr().cast_const(),
                init_ptr,
            )
        };
        let object = Object::from_raw(
            Arc::clone(self.vm.inner()),
            raw,
            "array creation",
            "failed to create java object array",
        )?;
        Ok(JavaArray::from_object(object))
    }

    /// wraps a raw memory region as a java.nio.ByteBuffer via NewDirectByteBuffer (missing safety doc for now)
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn new_direct_byte_buffer(
        &self,
        address: *mut u8,
        capacity: usize,
    ) -> Result<Object<'_>, Error> {
        let capacity = i64::try_from(capacity)
            .map_err(|_| Error::new("direct buffer creation", "capacity exceeds jlong max"))?;
        let raw = unsafe {
            ffi::usagi_vm_new_direct_byte_buffer(
                self.vm.raw_ptr(),
                address.cast::<c_void>(),
                capacity,
            )
        };
        Object::from_raw(
            Arc::clone(self.vm.inner()),
            raw,
            "direct buffer creation",
            "failed to create direct ByteBuffer",
        )
    }

    /// returns the base address of a direct java.nio.ByteBuffer via GetDirectBufferAddress
    pub fn get_direct_buffer_address(&self, buffer: &Object<'_>) -> Result<*mut u8, Error> {
        if !buffer.belongs_to_inner(self.vm.inner()) {
            return Err(Error::new(
                "direct buffer address",
                "buffer belongs to a different vm",
            ));
        }
        let mut address: *mut c_void = std::ptr::null_mut();
        let status = unsafe {
            ffi::usagi_direct_buffer_address(buffer.raw().as_ptr().cast_const(), &mut address)
        };
        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "direct buffer address",
                "failed to get direct buffer address",
            ));
        }
        Ok(address.cast::<u8>())
    }

    /// returns the capacity in bytes of a direct java.nio.ByteBuffer via GetDirectBufferCapacity
    pub fn get_direct_buffer_capacity(&self, buffer: &Object<'_>) -> Result<usize, Error> {
        if !buffer.belongs_to_inner(self.vm.inner()) {
            return Err(Error::new(
                "direct buffer capacity",
                "buffer belongs to a different vm",
            ));
        }
        let mut capacity: ffi::jlong = -1;
        let status = unsafe {
            ffi::usagi_direct_buffer_capacity(buffer.raw().as_ptr().cast_const(), &mut capacity)
        };
        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "direct buffer capacity",
                "failed to get direct buffer capacity",
            ));
        }
        usize::try_from(capacity).map_err(|_| {
            Error::new(
                "direct buffer capacity",
                "direct buffer capacity does not fit in usize",
            )
        })
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
