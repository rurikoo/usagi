#![allow(dead_code)]
pub use jni_sys::{JNIEnv, JavaVM, jboolean, jlong, jobject, jvalue};
use std::{
    ffi::{c_char, c_int, c_void},
    marker::{PhantomData, PhantomPinned},
};

/// jni's jint
/// hm should i remove this and just use jni_sys's jnit?
/// prolly no cuz they are same thing
pub type Jint = i32;

pub const JNI_OK: Jint = 0;
pub const JNI_ERR: Jint = -1;
pub const JNI_COMMIT: Jint = 1;
pub const JNI_ABORT: Jint = 2;

pub const USAGI_OK: Jint = 0;
pub const USAGI_ERROR: Jint = -1;
pub const USAGI_BUFFER_TOO_SMALL: Jint = -2;
pub const USAGI_EXCEPTION: Jint = -3;

pub const USAGI_OBJECT_KIND_OBJECT: c_int = 0;
pub const USAGI_OBJECT_KIND_CLASS: c_int = 1;
pub const USAGI_OBJECT_KIND_STRING: c_int = 2;
pub const USAGI_OBJECT_KIND_ARRAY: c_int = 3;

pub const USAGI_ARRAY_KIND_NONE: c_int = 0;
pub const USAGI_ARRAY_KIND_BOOLEAN: c_int = 1;
pub const USAGI_ARRAY_KIND_BYTE: c_int = 2;
pub const USAGI_ARRAY_KIND_CHAR: c_int = 3;
pub const USAGI_ARRAY_KIND_SHORT: c_int = 4;
pub const USAGI_ARRAY_KIND_INT: c_int = 5;
pub const USAGI_ARRAY_KIND_LONG: c_int = 6;
pub const USAGI_ARRAY_KIND_FLOAT: c_int = 7;
pub const USAGI_ARRAY_KIND_DOUBLE: c_int = 8;
pub const USAGI_ARRAY_KIND_OBJECT: c_int = 9;

pub type MethodCallback = unsafe extern "C" fn(
    user_data: *mut c_void,
    env: *mut JNIEnv,
    args: *const jvalue,
    out_value: *mut jvalue,
) -> Jint;

pub type MethodDrop = unsafe extern "C" fn(user_data: *mut c_void);

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

    pub fn usagi_vm_attach_current_thread(
        vm: *mut UsagiVm,
        out_env: *mut *mut JNIEnv,
        out_attached: *mut c_int,
    ) -> Jint;

    pub fn usagi_vm_detach_current_thread(vm: *mut UsagiVm) -> Jint;

    pub fn usagi_vm_generate_class_dump(vm: *mut UsagiVm, path: *const c_char) -> Jint;

    pub fn usagi_vm_find_class(vm: *mut UsagiVm, internal_name: *const c_char) -> *mut UsagiObject;

    pub fn usagi_vm_new_string_utf16(
        vm: *mut UsagiVm,
        units: *const u16,
        length: usize,
    ) -> *mut UsagiObject;

    pub fn usagi_object_destroy(object: *mut UsagiObject);

    pub fn usagi_object_kind(object: *const UsagiObject) -> c_int;
    pub fn usagi_object_array_kind(object: *const UsagiObject) -> c_int;
    pub fn usagi_object_jobject(object: *const UsagiObject) -> jobject;
    pub fn usagi_object_to_local_ref(env: *mut JNIEnv, object: *const UsagiObject) -> jobject;
    pub fn usagi_wrap_jobject(
        vm: *mut UsagiVm,
        env: *mut JNIEnv,
        ref_: jobject,
        expected_kind: c_int,
    ) -> *mut UsagiObject;

    pub fn usagi_vm_new_primitive_array(
        vm: *mut UsagiVm,
        array_kind: c_int,
        length: usize,
    ) -> *mut UsagiObject;
    pub fn usagi_vm_new_object_array(
        vm: *mut UsagiVm,
        length: usize,
        element_class: *const UsagiObject,
        init: *const UsagiObject,
    ) -> *mut UsagiObject;
    pub fn usagi_array_length(array: *const UsagiObject, out_length: *mut usize) -> Jint;
    pub fn usagi_object_array_get(
        array: *const UsagiObject,
        index: usize,
        out_element: *mut *mut UsagiObject,
    ) -> Jint;
    pub fn usagi_object_array_set(
        array: *mut UsagiObject,
        index: usize,
        value: *const UsagiObject,
    ) -> Jint;
    pub fn usagi_array_get_region(
        array: *const UsagiObject,
        element_kind: c_int,
        start: usize,
        length: usize,
        buffer: *mut c_void,
    ) -> Jint;
    pub fn usagi_array_set_region(
        array: *mut UsagiObject,
        element_kind: c_int,
        start: usize,
        length: usize,
        buffer: *const c_void,
    ) -> Jint;
    pub fn usagi_array_get_elements(
        array: *mut UsagiObject,
        element_kind: c_int,
        out_ptr: *mut *mut c_void,
        out_is_copy: *mut jboolean,
    ) -> Jint;
    pub fn usagi_array_release_elements(
        array: *mut UsagiObject,
        element_kind: c_int,
        ptr: *mut c_void,
        mode: Jint,
    ) -> Jint;

    pub fn usagi_vm_new_direct_byte_buffer(
        vm: *mut UsagiVm,
        address: *mut c_void,
        capacity: jlong,
    ) -> *mut UsagiObject;
    pub fn usagi_direct_buffer_address(
        buffer: *const UsagiObject,
        out_address: *mut *mut c_void,
    ) -> Jint;
    pub fn usagi_direct_buffer_capacity(
        buffer: *const UsagiObject,
        out_capacity: *mut jlong,
    ) -> Jint;

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

    pub fn usagi_class_register_method(
        class_object: *mut UsagiObject,
        name: *const c_char,
        signature: *const c_char,
        is_static: c_int,
        return_kind: c_char,
        user_data: *mut c_void,
        callback: Option<MethodCallback>,
        drop: Option<MethodDrop>,
    ) -> Jint;

    pub fn usagi_set_error(message: *const c_char);

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

    pub fn usagi_last_error() -> *const c_char;
}

#[cfg(test)]
mod tests {
    use crate::Vm;

    #[test]
    fn creates_and_destroys_vm() {
        let vm = Vm::new().expect("VM creation should succeed");
        let _env = vm.env().expect("JNI environment should be available");
    }

    #[test]
    fn finds_builtin_string_class() {
        let vm = Vm::new().expect("VM creation should succeed");
        let env = vm.env().expect("JNI env");

        let class = env
            .find_class("java/lang/String")
            .expect("string class lookup should succeed");

        drop(class);
    }

    #[test]
    fn roundtrips_unicode_and_embedded_nul() {
        let vm = Vm::new().expect("VM creation should succeed");
        let env = vm.env().expect("JNI env");
        let original = "Hello\0世界🦀";

        let java_string = env
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
        let env = vm.env().expect("JNI env");

        let java_string = env.new_string("").expect("empty string creation");

        assert_eq!(java_string.to_string_lossy().expect("conversion"), "");
    }

    #[test]
    fn rejects_invalid_class_names() {
        let vm = Vm::new().expect("VM creation should succeed");
        let env = vm.env().expect("JNI env");

        assert!(env.find_class("java\0/lang/String").is_err());
        assert!(env.find_class("").is_err());
    }

    #[test]
    fn survives_many_strings() {
        let vm = Vm::new().expect("VM creation should succeed");
        let env = vm.env().expect("JNI env");

        for i in 0..10_000 {
            let text = format!("string {i}");
            let java_string = env.new_string(&text).expect("string creation");

            assert_eq!(java_string.to_string_lossy().expect("conversion"), text);
        }
    }

    #[test]
    fn two_vms_coexist_on_one_thread() {
        let first = Vm::new().expect("first VM");
        let second = Vm::new().expect("second VM");

        let first_env = first.env().expect("first env");
        let second_env = second.env().expect("second env");

        let a = first_env.new_string("first").expect("string in first VM");
        let b = second_env
            .new_string("second")
            .expect("string in second VM");

        assert_eq!(a.to_string_lossy().expect("conversion"), "first");
        assert_eq!(b.to_string_lossy().expect("conversion"), "second");
    }

    #[test]
    fn unknown_class_is_created_on_demand() {
        let vm = Vm::new().expect("VM creation should succeed");
        let env = vm.env().expect("JNI env");

        // libjnivm creates classes on demand instead of failing
        assert!(env.find_class("does/not/Exist").is_ok());
    }
}
