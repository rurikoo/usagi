use std::{
    any::Any,
    ffi::{CString, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr::NonNull,
    sync::{Arc, Weak},
};

use crate::{
    array::JavaArray,
    class::Class,
    env::Env,
    error::Error,
    ffi::{self, jvalue},
    object::Object,
    sig::{Signature, Type},
    string::JavaString,
    value::{Value, fits},
    vm::{Vm, VmInner},
};

pub(crate) type Callback = Box<
    dyn for<'a, 'vm> Fn(&'a mut Env<'vm>, &[Value<'a>]) -> Result<Value<'a>, Error>
        + Send
        + Sync
        + 'static,
>;

struct Holder {
    vm: Weak<VmInner>,
    signature: Signature,
    callback: Callback,
}

pub(crate) fn into_raw(vm: Weak<VmInner>, signature: Signature, callback: Callback) -> *mut c_void {
    Box::into_raw(Box::new(Holder {
        vm,
        signature,
        callback,
    }))
    .cast()
}

pub(crate) unsafe extern "C" fn trampoline(
    user_data: *mut c_void,
    raw_env: *mut ffi::JNIEnv,
    args: *const jvalue,
    out_value: *mut jvalue,
) -> ffi::Jint {
    if user_data.is_null() {
        return fail(Error::new("callback", "null callback user_data"));
    }

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), Error> {
        let holder = unsafe { &*user_data.cast::<Holder>() };
        let Some(vm_inner) = holder.vm.upgrade() else {
            return Err(Error::new("callback", "VM has already been destroyed"));
        };
        let vm = Vm::from_inner(vm_inner);
        let raw_env_nn =
            NonNull::new(raw_env).ok_or_else(|| Error::new("callback", "null JNIEnv pointer"))?;
        let mut env = Env::new(&vm, raw_env_nn);

        let expected_args = holder.signature.args();

        if !expected_args.is_empty() && args.is_null() {
            return Err(Error::new("callback", "null arguments pointer"));
        }

        if *holder.signature.ret() != Type::Void && out_value.is_null() {
            return Err(Error::new("callback", "null return output pointer"));
        }

        let mut values = Vec::with_capacity(expected_args.len());
        for (index, ty) in expected_args.iter().enumerate() {
            values.push(unsafe { read_arg(&vm, raw_env, *args.add(index), ty) }?);
        }

        let value = (holder.callback)(&mut env, &values)?;
        unsafe { write_return(&vm, raw_env, holder.signature.ret(), value, out_value) }
    }));

    match result {
        Ok(Ok(())) => ffi::USAGI_OK,
        Ok(Err(error)) => fail(error),
        Err(payload) => fail(Error::new(
            "callback panic",
            panic_message(payload.as_ref()),
        )),
    }
}

pub(crate) unsafe extern "C" fn drop_holder(user_data: *mut c_void) {
    if !user_data.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            drop(unsafe { Box::from_raw(user_data.cast::<Holder>()) });
        }));
    }
}

unsafe fn read_arg<'vm>(
    vm: &'vm Vm,
    raw_env: *mut ffi::JNIEnv,
    value: jvalue,
    ty: &Type,
) -> Result<Value<'vm>, Error> {
    match ty {
        Type::Void => Err(Error::new("callback", "void is not a valid argument type")),
        Type::Boolean => Ok(Value::Boolean(unsafe { value.b != 0 })),
        Type::Byte => Ok(Value::Byte(unsafe { value.b })),
        Type::Char => Ok(Value::Char(unsafe { value.c })),
        Type::Short => Ok(Value::Short(unsafe { value.s })),
        Type::Int => Ok(Value::Int(unsafe { value.i })),
        Type::Long => Ok(Value::Long(unsafe { value.j })),
        Type::Float => Ok(Value::Float(unsafe { value.f })),
        Type::Double => Ok(Value::Double(unsafe { value.d })),
        Type::Object(name) => {
            let jobj = unsafe { value.l };
            if jobj.is_null() {
                return Ok(Value::Null);
            }

            let expected_kind = match name.as_str() {
                "java/lang/String" => ffi::USAGI_OBJECT_KIND_STRING,
                "java/lang/Class" => ffi::USAGI_OBJECT_KIND_CLASS,
                _ => ffi::USAGI_OBJECT_KIND_OBJECT,
            };

            let raw =
                unsafe { ffi::usagi_wrap_jobject(vm.raw_ptr(), raw_env, jobj, expected_kind) };
            let object = Object::from_raw(
                Arc::clone(vm.inner()),
                raw,
                "callback argument",
                "failed to wrap java object argument",
            )?;

            match name.as_str() {
                "java/lang/String" => Ok(Value::String(JavaString::from_object(object))),
                "java/lang/Class" => Ok(Value::Class(Class::from_object(object))),
                _ => Ok(Value::Object(object)),
            }
        }
        Type::Array(element) => {
            let jobj = unsafe { value.l };
            if jobj.is_null() {
                return Ok(Value::Null);
            }

            let raw = unsafe {
                ffi::usagi_wrap_jobject(vm.raw_ptr(), raw_env, jobj, ffi::USAGI_OBJECT_KIND_ARRAY)
            };
            let object = Object::from_raw(
                Arc::clone(vm.inner()),
                raw,
                "callback argument",
                "failed to wrap java array argument",
            )?;
            let array = JavaArray::from_object(object);
            if !array.kind().matches_type(element) {
                return Err(Error::new(
                    "callback argument",
                    "java array argument has mismatched element type",
                ));
            }
            Ok(Value::Array(array))
        }
    }
}

unsafe fn write_return(
    vm: &Vm,
    raw_env: *mut ffi::JNIEnv,
    ret: &Type,
    value: Value<'_>,
    out: *mut jvalue,
) -> Result<(), Error> {
    if matches!((ret, &value), (Type::Void, Value::Void)) {
        return Ok(());
    }

    if !fits(&value, ret) {
        return Err(Error::new(
            "callback",
            format!("expected {} return value, got {}", ret.name(), value.name()),
        ));
    }

    if let Some(object) = value.as_object() {
        if !object.belongs_to_inner(vm.inner()) {
            return Err(Error::new(
                "callback",
                "return object belongs to a different vm",
            ));
        }

        let local =
            unsafe { ffi::usagi_object_to_local_ref(raw_env, object.raw().as_ptr().cast_const()) };
        if local.is_null() {
            return Err(Error::from_native(
                "callback",
                "failed to convert return object to JNI local reference",
            ));
        }
        unsafe { (*out).l = local };
        return Ok(());
    }

    match value {
        Value::Boolean(v) => unsafe { (*out).z = v },
        Value::Byte(v) => unsafe { (*out).b = v },
        Value::Char(v) => unsafe { (*out).c = v },
        Value::Short(v) => unsafe { (*out).s = v },
        Value::Int(v) => unsafe { (*out).i = v },
        Value::Long(v) => unsafe { (*out).j = v },
        Value::Float(v) => unsafe { (*out).f = v },
        Value::Double(v) => unsafe { (*out).d = v },
        Value::Null => unsafe { (*out).l = std::ptr::null_mut() },
        Value::Void | Value::Object(_) | Value::String(_) | Value::Class(_) | Value::Array(_) => {
            unreachable!()
        }
    }

    Ok(())
}

fn fail(error: Error) -> ffi::Jint {
    let message = error.to_string().replace('\0', " ");

    if let Ok(message) = CString::new(message) {
        unsafe { ffi::usagi_set_error(message.as_ptr()) };
    }

    ffi::USAGI_ERROR
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "callback panicked".to_owned()
    }
}
