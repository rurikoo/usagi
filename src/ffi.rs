#![allow(dead_code)]
pub use jni_sys::{JNIEnv, JavaVM, jvalue};
use std::{
    ffi::c_char,
    ffi::c_int,
    ffi::c_void,
    marker::{PhantomData, PhantomPinned},
};

/// jni's jint
/// hm should i remove this and just use jni_sys's jnit?
/// prolly no cuz they are same thing
pub type Jint = i32;

pub const JNI_OK: Jint = 0;
pub const JNI_ERR: Jint = -1;

pub const USAGI_OK: Jint = 0;
pub const USAGI_ERROR: Jint = -1;
pub const USAGI_BUFFER_TOO_SMALL: Jint = -2;
pub const USAGI_EXCEPTION: Jint = -3;

#[repr(C)]
pub struct UsagiVm {
    _data: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

#[repr(C)]
pub struct UsagiObject {
    _data: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

unsafe extern "C" {
    pub fn usagi_vm_create() -> *mut UsagiVm;
    pub fn usagi_vm_destroy(vm: *mut UsagiVm);

    pub fn usagi_vm_get_java_vm(vm: *mut UsagiVm) -> *mut JavaVM;
    pub fn usagi_vm_get_jni_env(vm: *mut UsagiVm) -> *mut JNIEnv;

    pub fn usagi_vm_attach_current_thread(vm: *mut UsagiVm, out_env: *mut *mut JNIEnv) -> Jint;

    pub fn usagi_vm_detach_current_thread(vm: *mut UsagiVm) -> Jint;

    pub fn usagi_vm_generate_class_dump(vm: *mut UsagiVm, path: *const c_char) -> Jint;

    pub fn usagi_vm_find_class(vm: *mut UsagiVm, internal_name: *const c_char) -> *mut UsagiObject;

    pub fn usagi_vm_new_string_utf16(
        vm: *mut UsagiVm,
        units: *const u16,
        length: usize,
    ) -> *mut UsagiObject;

    pub fn usagi_object_destroy(object: *mut UsagiObject);

    pub fn usagi_string_utf16_length(object: *const UsagiObject, out_length: *mut usize) -> Jint;

    pub fn usagi_string_copy_utf16(
        object: *const UsagiObject,
        buffer: *mut u16,
        capacity: usize,
        out_length: *mut usize,
    ) -> Jint;

    pub fn usagi_class_get_method(
        class_object: *const UsagiObject,
        name: *const c_char,
        signature: *const c_char,
        is_static: c_int,
    ) -> *mut c_void;

    pub fn usagi_call_method(
        class_object: *const UsagiObject,
        receiver: *const UsagiObject,
        method_id: *mut c_void,
        is_static: c_int,
        return_kind: c_char,
        args: *mut jvalue,
        out_value: *mut jvalue,
        out_object_kind: c_int,
        out_object: *mut *mut UsagiObject,
    ) -> Jint;

    pub fn usagi_test_register_add(vm: *mut UsagiVm) -> Jint;
    pub fn usagi_test_register_fail(vm: *mut UsagiVm) -> Jint;

    pub fn usagi_last_error() -> *const c_char;
}

#[cfg(test)]
mod tests {
    use crate::Vm;

    #[test]
    fn creates_and_destroys_vm() {
        let vm = Vm::new().expect("VM creation should succeed");

        vm.get_jni_env()
            .expect("JNI environment should be available");
    }

    #[test]
    fn finds_builtin_string_class() {
        let vm = Vm::new().expect("VM creation should succeed");

        let class = vm
            .find_class("java/lang/String")
            .expect("string class lookup should succeed");

        drop(class);
    }

    #[test]
    fn roundtrips_unicode_and_embedded_nul() {
        let vm = Vm::new().expect("VM creation should succeed");
        let original = "Hello\0世界🦀";

        let java_string = vm
            .new_string(original)
            .expect("string creation should succeed");

        assert_eq!(
            java_string.to_string_lossy().expect("string conversion"),
            original
        );
    }

    #[test]
    fn roundtrips_empty_string() {
        let vm = Vm::new().expect("VM creation should succeed");

        let java_string = vm.new_string("").expect("empty string creation");

        assert_eq!(java_string.to_string_lossy().expect("conversion"), "");
    }

    #[test]
    fn rejects_invalid_class_names() {
        let vm = Vm::new().expect("VM creation should succeed");

        assert!(vm.find_class("java\0/lang/String").is_err());
        assert!(vm.find_class("").is_err());
    }

    #[test]
    fn survives_many_strings() {
        let vm = Vm::new().expect("VM creation should succeed");

        for i in 0..10_000 {
            let text = format!("string {i}");
            let java_string = vm.new_string(&text).expect("string creation");

            assert_eq!(java_string.to_string_lossy().expect("conversion"), text);
        }
    }

    #[test]
    fn two_vms_coexist_on_one_thread() {
        let first = Vm::new().expect("first VM");
        let second = Vm::new().expect("second VM");

        let a = first.new_string("first").expect("string in first VM");
        let b = second.new_string("second").expect("string in second VM");

        assert_eq!(a.to_string_lossy().expect("conversion"), "first");
        assert_eq!(b.to_string_lossy().expect("conversion"), "second");
    }

    #[test]
    fn unknown_class_is_created_on_demand() {
        let vm = Vm::new().expect("VM creation should succeed");

        // libjnivm creates classes on demand instead of failing
        assert!(vm.find_class("does/not/Exist").is_ok());
    }
}
