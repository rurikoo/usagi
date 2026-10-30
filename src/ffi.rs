#![allow(dead_code)]
use std::{
    ffi::c_char,
    marker::{PhantomData, PhantomPinned},
};

/// jni's jint
pub type Jint = i32;

pub const JNI_OK: Jint = 0;
pub const JNI_ERR: Jint = -1;

pub const USAGI_OK: Jint = 0;
pub const USAGI_ERROR: Jint = -1;
pub const USAGI_BUFFER_TOO_SMALL: Jint = -2;

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

#[repr(C)]
pub struct JavaVM {
    _data: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

#[repr(C)]
pub struct JNIEnv {
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

    pub fn usagi_last_error() -> *const c_char;
}
