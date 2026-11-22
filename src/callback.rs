use std::{
    any::Any,
    ffi::{CString, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
};

use crate::{
    error::Error,
    ffi::{self, jvalue},
    sig::{Signature, Type},
    value::Value,
};

pub(crate) type Callback = Box<dyn Fn(&[Value]) -> Result<Value, Error> + Send + Sync + 'static>;

struct Holder {
    signature: Signature,
    callback: Callback,
}

pub(crate) fn into_raw(signature: Signature, callback: Callback) -> *mut c_void {
    Box::into_raw(Box::new(Holder {
        signature,
        callback,
    }))
    .cast()
}

pub(crate) unsafe extern "C" fn trampoline(
    user_data: *mut c_void,
    args: *const jvalue,
    out_value: *mut jvalue,
) -> ffi::Jint {
    if user_data.is_null() {
        return fail(Error::new("callback", "null callback user_data"));
    }

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), Error> {
        let holder = unsafe { &*user_data.cast::<Holder>() };
        let expected_args = holder.signature.args();

        if !expected_args.is_empty() && args.is_null() {
            return Err(Error::new("callback", "null arguments pointer"));
        }

        if *holder.signature.ret() != Type::Void && out_value.is_null() {
            return Err(Error::new("callback", "null return output pointer"));
        }

        let values = expected_args
            .iter()
            .enumerate()
            .map(|(index, ty)| unsafe { read_arg(*args.add(index), ty) })
            .collect::<Vec<_>>();

        let value = (holder.callback)(&values)?;
        unsafe { write_return(holder.signature.ret(), value, out_value) }
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

unsafe fn read_arg(value: jvalue, ty: &Type) -> Value {
    match ty {
        Type::Boolean => Value::Boolean(unsafe { value.b != 0 }),
        Type::Byte => Value::Byte(unsafe { value.b }),
        Type::Char => Value::Char(unsafe { value.c }),
        Type::Short => Value::Short(unsafe { value.s }),
        Type::Int => Value::Int(unsafe { value.i }),
        Type::Long => Value::Long(unsafe { value.j }),
        Type::Float => Value::Float(unsafe { value.f }),
        Type::Double => Value::Double(unsafe { value.d }),
        // registration rejects these, so this arm is unreachable
        _ => Value::Null,
    }
}

unsafe fn write_return(ret: &Type, value: Value, out: *mut jvalue) -> Result<(), Error> {
    match (ret, value) {
        (Type::Void, Value::Void) => {}
        (Type::Boolean, Value::Boolean(value)) => unsafe { (*out).z = value },
        (Type::Byte, Value::Byte(value)) => unsafe { (*out).b = value },
        (Type::Char, Value::Char(value)) => unsafe { (*out).c = value },
        (Type::Short, Value::Short(value)) => unsafe { (*out).s = value },
        (Type::Int, Value::Int(value)) => unsafe { (*out).i = value },
        (Type::Long, Value::Long(value)) => unsafe { (*out).j = value },
        (Type::Float, Value::Float(value)) => unsafe { (*out).f = value },
        (Type::Double, Value::Double(value)) => unsafe { (*out).d = value },
        _ => {
            return Err(Error::new(
                "callback",
                format!("expected {} return value, got {}", ret.name(), value.name()),
            ));
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
