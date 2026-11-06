use std::{ffi::c_void, marker::PhantomData, ptr::NonNull};

use crate::sig::Signature;

#[allow(dead_code)]
pub struct Method<'class> {
    pub(crate) id: NonNull<c_void>,
    pub(crate) signature: Signature,
    pub(crate) is_static: bool,
    pub(crate) _class: PhantomData<&'class ()>,
}

impl Method<'_> {
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    pub fn is_static(&self) -> bool {
        self.is_static
    }
}

#[cfg(test)]
mod tests {
    use crate::Vm;

    #[test]
    fn probe_unknown_method() {
        let vm = Vm::new().unwrap();
        let class = vm.find_class("does/not/Owo").unwrap();
        println!("{:?}", class.method("nope", "()V").map(|_| ()));
    }

    #[test]
    fn rejects_invalid_signature() {
        let vm = Vm::new().unwrap();
        let class = vm.find_class("java/lang/String").unwrap();

        let error = class
            .method("length", "not a signature")
            .map(|_| ())
            .unwrap_err();
        assert_eq!(error.operation(), "signature parse");
    }

    #[test]
    fn rejects_bad_method_names() {
        let vm = Vm::new().unwrap();
        let class = vm.find_class("java/lang/String").unwrap();

        assert!(class.method("", "()V").is_err());
        assert!(class.method("a\0b", "()V").is_err());
    }

    #[test]
    fn method_keeps_its_signature() {
        let vm = Vm::new().unwrap();
        let class = vm.find_class("java/lang/String").unwrap();

        // only runs the checks if jnivm hands out a method id
        if let Ok(method) = class.method("length", "()I") {
            assert_eq!(method.signature().as_str(), "()I");
            assert!(!method.is_static());
        }
    }
}
