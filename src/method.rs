use std::{
    ffi::{c_char, c_int, c_void},
    ptr::NonNull,
};

use crate::{
    error::Error,
    ffi::{self, jvalue},
    object::Object,
    sig::{Signature, Type},
    string::JavaString,
    value::{Value, check_args},
    vm::Vm,
};

const KIND_OBJECT: c_int = 0;
const KIND_STRING: c_int = 2;

pub struct Method<'class> {
    pub(crate) id: NonNull<c_void>,
    pub(crate) signature: Signature,
    pub(crate) is_static: bool,
    pub(crate) class: NonNull<ffi::UsagiObject>,
    pub(crate) vm: &'class Vm,
}

/// the result of a call, typed by the method's return type
pub enum Output<'vm> {
    Void,
    Boolean(bool),
    Byte(i8),
    Char(u16),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    /// a non-string object or array, None for null
    Object(Option<Object<'vm>>),
    /// a java string, None for null
    String(Option<JavaString<'vm>>),
}

impl<'class> Method<'class> {
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    pub fn is_static(&self) -> bool {
        self.is_static
    }

    /// calls the method, static methods take None as the receiver
    pub fn call(
        &self,
        receiver: Option<&Object<'_>>,
        args: &[Value],
    ) -> Result<Output<'class>, Error> {
        // all argument checks happen before any native call
        check_args(args, self.signature.args())?;

        let receiver_ptr = match (self.is_static, receiver) {
            (true, None) => std::ptr::null(),
            (true, Some(_)) => {
                return Err(Error::new("method call", "static method takes no receiver"));
            }
            (false, None) => {
                return Err(Error::new(
                    "method call",
                    "instance method needs a receiver",
                ));
            }
            (false, Some(object)) => {
                if !object.belongs_to(self.vm) {
                    return Err(Error::new(
                        "method call",
                        "receiver belongs to a different vm",
                    ));
                }
                object.raw.as_ptr().cast_const()
            }
        };

        let mut jargs: Vec<jvalue> = args.iter().map(to_jvalue).collect();
        let ret = self.signature.ret();
        let kind = match ret {
            Type::Object(name) if name.as_str() == "java/lang/String" => KIND_STRING,
            _ => KIND_OBJECT,
        };

        let mut value = jvalue { j: 0 };
        let mut out_object: *mut ffi::UsagiObject = std::ptr::null_mut();

        let status = unsafe {
            ffi::usagi_call_method(
                self.class.as_ptr().cast_const(),
                receiver_ptr,
                self.id.as_ptr(),
                c_int::from(self.is_static),
                return_code(ret) as c_char,
                jargs.as_mut_ptr(),
                &mut value,
                kind,
                &mut out_object,
            )
        };

        if status != ffi::USAGI_OK {
            return Err(Error::from_native("method call", "java method call failed"));
        }

        let object = self.reference(out_object)?;

        Ok(unsafe {
            match ret {
                Type::Void => Output::Void,
                Type::Boolean => Output::Boolean(value.z),
                Type::Byte => Output::Byte(value.b),
                Type::Char => Output::Char(value.c),
                Type::Short => Output::Short(value.s),
                Type::Int => Output::Int(value.i),
                Type::Long => Output::Long(value.j),
                Type::Float => Output::Float(value.f),
                Type::Double => Output::Double(value.d),
                Type::Object(name) if name.as_str() == "java/lang/String" => {
                    Output::String(object.map(JavaString::from_object))
                }
                Type::Object(_) | Type::Array(_) => Output::Object(object),
            }
        })
    }

    /// wraps a handle from the bridge, None when the java result was null
    fn reference(&self, raw: *mut ffi::UsagiObject) -> Result<Option<Object<'class>>, Error> {
        if raw.is_null() {
            return Ok(None);
        }

        Object::from_raw(
            self.vm,
            raw,
            "method call",
            "java method returned an invalid object",
        )
        .map(Some)
    }
}

fn to_jvalue(value: &Value) -> jvalue {
    match *value {
        Value::Boolean(v) => jvalue { z: v },
        Value::Byte(v) => jvalue { b: v },
        Value::Char(v) => jvalue { c: v },
        Value::Short(v) => jvalue { s: v },
        Value::Int(v) => jvalue { i: v },
        Value::Long(v) => jvalue { j: v },
        Value::Float(v) => jvalue { f: v },
        Value::Double(v) => jvalue { d: v },
        Value::Null => jvalue {
            l: std::ptr::null_mut(),
        },
    }
}

/// the jni type character for a return type, as the bridge expects it
fn return_code(ty: &Type) -> u8 {
    match ty {
        Type::Void => b'V',
        Type::Boolean => b'Z',
        Type::Byte => b'B',
        Type::Char => b'C',
        Type::Short => b'S',
        Type::Int => b'I',
        Type::Long => b'J',
        Type::Float => b'F',
        Type::Double => b'D',
        Type::Object(_) | Type::Array(_) => b'L',
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi;

    fn register_add(vm: &Vm) {
        let status = unsafe { ffi::usagi_test_register_add(vm.raw_ptr()) };
        assert_eq!(status, ffi::USAGI_OK);
    }

    #[test]
    fn add_returns_the_sum_of_its_arguments() {
        let vm = Vm::new().unwrap();
        register_add(&vm);

        let class = vm.find_class("usagi/Test").unwrap();
        let add = class.static_method("add", "(II)I").unwrap();

        match add.call(None, &[Value::Int(3), Value::Int(4)]).unwrap() {
            Output::Int(sum) => assert_eq!(sum, 7),
            _ => panic!("add should return an int"),
        }
    }

    #[test]
    fn add_rejects_bad_arguments_before_calling_java() {
        let vm = Vm::new().unwrap();
        register_add(&vm);

        let class = vm.find_class("usagi/Test").unwrap();
        let add = class.static_method("add", "(II)I").unwrap();

        assert!(add.call(None, &[Value::Long(3), Value::Int(4)]).is_err());
        assert!(add.call(None, &[Value::Int(3)]).is_err());
    }

    #[test]
    fn static_call_rejects_a_receiver() {
        let vm = Vm::new().unwrap();
        register_add(&vm);

        let class = vm.find_class("usagi/Test").unwrap();
        let add = class.static_method("add", "(II)I").unwrap();
        let text = vm.new_string("x").unwrap();

        assert!(
            add.call(Some(text.object()), &[Value::Int(3), Value::Int(4)])
                .is_err()
        );
    }
}
