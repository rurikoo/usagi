use std::{marker::PhantomData, ptr::NonNull, rc::Rc, sync::Arc};

use crate::{
    class::Class,
    env::Env,
    error::Error,
    ffi,
    string::JavaString,
    vm::{Vm, VmInner},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObjectKind {
    Object,
    Class,
    String,
}

pub(crate) struct OwnedObject {
    pub(crate) raw: NonNull<ffi::UsagiObject>,
    pub(crate) vm: Arc<VmInner>,
    pub(crate) kind: ObjectKind,
}

unsafe impl Send for OwnedObject {}
unsafe impl Sync for OwnedObject {}

impl Drop for OwnedObject {
    fn drop(&mut self) {
        unsafe {
            ffi::usagi_object_destroy(self.raw.as_ptr());
        }
    }
}

pub struct Object<'env> {
    inner: Arc<OwnedObject>,
    _marker: PhantomData<(&'env (), Rc<()>)>,
}

impl<'env> Object<'env> {
    pub(crate) fn from_raw(
        vm: Arc<VmInner>,
        raw: *mut ffi::UsagiObject,
        kind: ObjectKind,
        operation: &'static str,
        fallback: &str,
    ) -> Result<Self, Error> {
        let raw = NonNull::new(raw).ok_or_else(|| Error::from_native(operation, fallback))?;

        Ok(Self {
            inner: Arc::new(OwnedObject { raw, vm, kind }),
            _marker: PhantomData,
        })
    }

    pub(crate) fn from_owned(inner: Arc<OwnedObject>) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }

    pub(crate) fn raw(&self) -> NonNull<ffi::UsagiObject> {
        self.inner.raw
    }

    pub(crate) fn vm_inner(&self) -> &Arc<VmInner> {
        &self.inner.vm
    }

    pub(crate) fn belongs_to_inner(&self, vm: &Arc<VmInner>) -> bool {
        Arc::ptr_eq(&self.inner.vm, vm)
    }

    /// true when this object belongs to vm
    pub fn belongs_to(&self, vm: &Vm) -> bool {
        vm.same_vm(&self.inner.vm)
    }

    /// promotes a clone of this handle to GlobalRef
    pub fn to_global(&self) -> GlobalRef {
        GlobalRef {
            inner: Arc::clone(&self.inner),
        }
    }

    /// converts this handle into GlobalRef
    pub fn into_global(self) -> GlobalRef {
        GlobalRef { inner: self.inner }
    }
}

/// static java global reference that can be shared across threads
#[derive(Clone)]
pub struct GlobalRef {
    inner: Arc<OwnedObject>,
}

impl GlobalRef {
    pub fn belongs_to(&self, vm: &Vm) -> bool {
        vm.same_vm(&self.inner.vm)
    }

    /// binds this global reference to an attached Env as an Object
    pub fn as_object<'env>(&self, env: &'env Env<'_>) -> Result<Object<'env>, Error> {
        if !env.vm().same_vm(&self.inner.vm) {
            return Err(Error::new(
                "global reference",
                "global reference belongs to a different vm",
            ));
        }

        Ok(Object::from_owned(Arc::clone(&self.inner)))
    }

    /// binds this global reference as a Class if it refers to a java class
    pub fn as_class<'env>(&self, env: &'env Env<'_>) -> Result<Class<'env>, Error> {
        if self.inner.kind != ObjectKind::Class {
            return Err(Error::new(
                "global reference",
                "global reference is not a Java class",
            ));
        }

        Ok(Class::from_object(self.as_object(env)?))
    }

    /// binds this global reference as a JavaString if it refers to a Java string
    pub fn as_string<'env>(&self, env: &'env Env<'_>) -> Result<JavaString<'env>, Error> {
        if self.inner.kind != ObjectKind::String {
            return Err(Error::new(
                "global reference",
                "global reference is not a Java string",
            ));
        }

        Ok(JavaString::from_object(self.as_object(env)?))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::Vm;

    #[test]
    fn objects_belong_only_to_their_vm() {
        let a = Vm::new().unwrap();
        let b = Vm::new().unwrap();

        let env_a = a.env().unwrap();
        let string = env_a.new_string("hi").unwrap();

        assert!(string.as_object().belongs_to(&a));
        assert!(!string.as_object().belongs_to(&b));
    }

    #[test]
    fn global_ref_works_across_threads_and_drops_on_detached_thread() {
        let vm = Arc::new(Vm::new().unwrap());
        let vm_worker = Arc::clone(&vm);

        let global = std::thread::spawn(move || {
            let guard = vm_worker.attach_current_thread().unwrap();
            guard.new_string("from worker").unwrap().into_global()
        })
        .join()
        .unwrap();

        let env = vm.env().unwrap();
        let bound = global.as_string(&env).unwrap();
        assert_eq!(bound.to_string_lossy().unwrap(), "from worker");
        drop(bound);

        std::thread::spawn(move || drop(global)).join().unwrap();
    }
}
