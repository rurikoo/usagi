use std::{
    ffi::{c_char, c_int, c_void},
    marker::PhantomData,
    ptr::NonNull,
    sync::Arc,
};

use crate::{
    array::JavaArray,
    class::Class,
    error::Error,
    ffi::{self, jvalue},
    object::Object,
    sig::{Signature, Type},
    string::JavaString,
    value::{Value, check_args},
    vm::VmInner,
};

pub struct Method<'class, 'env> {
    pub(crate) id: NonNull<c_void>,
    pub(crate) signature: Signature,
    pub(crate) is_static: bool,
    pub(crate) class: NonNull<ffi::UsagiObject>,
    pub(crate) vm: Arc<VmInner>,
    pub(crate) _class: PhantomData<&'class Class<'env>>,
}

/// the result of a call, typed by the method's return type
pub enum Output<'env> {
    Void,
    Boolean(bool),
    Byte(i8),
    Char(u16),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    /// a non-string, non-class, non-array object, None for null
    Object(Option<Object<'env>>),
    /// a java string, None for null
    String(Option<JavaString<'env>>),
    /// a java class, None for null
    Class(Option<Class<'env>>),
    /// a java array, None for null
    Array(Option<JavaArray<'env>>),
}

impl<'class, 'env> Method<'class, 'env> {
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
        args: &[Value<'_>],
    ) -> Result<Output<'env>, Error> {
        // all argument checks happen before any native call
        check_args(args, self.signature.args())?;

        for (index, arg) in args.iter().enumerate() {
            if let Some(object) = arg.as_object() {
                if !object.belongs_to_inner(&self.vm) {
                    return Err(Error::new(
                        "argument check",
                        format!("argument {index} belongs to a different vm"),
                    ));
                }
            }
        }

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
                if !object.belongs_to_inner(&self.vm) {
                    return Err(Error::new(
                        "method call",
                        "receiver belongs to a different vm",
                    ));
                }
                object.raw().as_ptr().cast_const()
            }
        };

        let mut jargs: Vec<jvalue> = args.iter().map(to_jvalue).collect();
        let args_ptr = if jargs.is_empty() {
            std::ptr::null_mut()
        } else {
            jargs.as_mut_ptr()
        };

        let ret = self.signature.ret();
        let ffi_kind = match ret {
            Type::Object(name) if name.as_str() == "java/lang/String" => {
                ffi::USAGI_OBJECT_KIND_STRING
            }
            Type::Object(name) if name.as_str() == "java/lang/Class" => {
                ffi::USAGI_OBJECT_KIND_CLASS
            }
            Type::Array(_) => ffi::USAGI_OBJECT_KIND_ARRAY,
            _ => ffi::USAGI_OBJECT_KIND_OBJECT,
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
                args_ptr,
                &mut value,
                ffi_kind,
                &mut out_object,
            )
        };

        if status != ffi::USAGI_OK {
            let operation = if status == ffi::USAGI_EXCEPTION {
                "java exception"
            } else {
                "method call"
            };
            return Err(Error::from_native(operation, "java method call failed"));
        }

        let object = self.reference(out_object)?;

        Ok(unsafe {
            match ret {
                Type::Void => Output::Void,
                Type::Boolean => Output::Boolean(value.b != 0),
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
                Type::Object(name) if name.as_str() == "java/lang/Class" => {
                    Output::Class(object.map(Class::from_object))
                }
                Type::Array(element) => {
                    let array = object.map(JavaArray::from_object);
                    if let Some(ref arr) = array {
                        if !arr.kind().matches_type(element) {
                            return Err(Error::new(
                                "method call",
                                "returned java array has mismatched element type",
                            ));
                        }
                    }
                    Output::Array(array)
                }
                Type::Object(_) => Output::Object(object),
            }
        })
    }

    /// wraps a handle from the bridge, None when the java result was null
    fn reference(&self, raw: *mut ffi::UsagiObject) -> Result<Option<Object<'env>>, Error> {
        if raw.is_null() {
            return Ok(None);
        }

        Object::from_raw(
            Arc::clone(&self.vm),
            raw,
            "method call",
            "java method returned an invalid object",
        )
        .map(Some)
    }
}

fn to_jvalue(value: &Value<'_>) -> jvalue {
    if let Some(object) = value.as_object() {
        return jvalue {
            l: unsafe { ffi::usagi_object_jobject(object.raw().as_ptr().cast_const()) },
        };
    }

    match *value {
        Value::Void => jvalue { j: 0 },
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
        Value::Object(_) | Value::String(_) | Value::Class(_) | Value::Array(_) => unreachable!(),
    }
}

/// the jni type character for a return type, as the bridge expects it
pub(crate) fn return_code(ty: &Type) -> u8 {
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
        Type::Object(_) => b'L',
        Type::Array(_) => b'[',
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Env, Vm};

    fn register_add(env: &Env<'_>) {
        env.define_class("usagi/Test")
            .unwrap()
            .register_static_method("add", "(II)I", |_env, args| match args {
                [Value::Int(a), Value::Int(b)] => Ok(Value::Int(a + b)),
                _ => Err(Error::new("callback", "unexpected arguments")),
            })
            .unwrap();
    }

    fn register_fail(env: &Env<'_>) {
        env.define_class("usagi/Test")
            .unwrap()
            .register_static_method("fail", "()I", |_, _| -> Result<Value<'_>, Error> {
                Err(Error::new("callback", "usagi test failure"))
            })
            .unwrap();
    }

    #[test]
    fn add_returns_the_sum_of_its_arguments() {
        let vm = Vm::new().unwrap();
        let env = vm.env().unwrap();
        register_add(&env);

        let class = env.find_class("usagi/Test").unwrap();
        let add = class.static_method("add", "(II)I").unwrap();

        match add.call(None, &[Value::Int(3), Value::Int(4)]).unwrap() {
            Output::Int(sum) => assert_eq!(sum, 7),
            _ => panic!("add should return an int"),
        }
    }

    #[test]
    fn call_output_can_outlive_temporary_class() {
        let vm = Vm::new().unwrap();
        let env = vm.env().unwrap();
        register_add(&env);

        let result: Output<'_> = {
            let class = env.find_class("usagi/Test").unwrap();
            let add = class.static_method("add", "(II)I").unwrap();
            add.call(None, &[Value::Int(10), Value::Int(20)]).unwrap()
        };

        assert!(matches!(result, Output::Int(30)));
    }

    #[test]
    fn add_rejects_bad_arguments_before_calling_java() {
        let vm = Vm::new().unwrap();
        let env = vm.env().unwrap();
        register_add(&env);

        let class = env.find_class("usagi/Test").unwrap();
        let add = class.static_method("add", "(II)I").unwrap();

        assert!(add.call(None, &[Value::Long(3), Value::Int(4)]).is_err());
        assert!(add.call(None, &[Value::Int(3)]).is_err());
    }

    #[test]
    fn static_call_rejects_a_receiver() {
        let vm = Vm::new().unwrap();
        let env = vm.env().unwrap();
        register_add(&env);

        let class = env.find_class("usagi/Test").unwrap();
        let add = class.static_method("add", "(II)I").unwrap();
        let text = env.new_string("x").unwrap();

        assert!(
            add.call(Some(text.as_object()), &[Value::Int(3), Value::Int(4)])
                .is_err()
        );
    }

    #[test]
    fn thrown_exception_becomes_an_error() {
        let vm = Vm::new().unwrap();
        let env = vm.env().unwrap();
        register_fail(&env);

        let class = env.find_class("usagi/Test").unwrap();
        let fail = class.static_method("fail", "()I").unwrap();

        let error = fail.call(None, &[]).map(|_| ()).unwrap_err();
        assert_eq!(error.operation(), "java exception");
        assert!(
            error.message().contains("usagi test failure"),
            "{}",
            error.message()
        );
    }

    #[test]
    fn callback_panic_becomes_an_error_and_calls_continue() {
        let vm = Vm::new().unwrap();
        let env = vm.env().unwrap();
        register_add(&env);

        env.define_class("usagi/Test")
            .unwrap()
            .register_static_method("panic", "()I", |_, _| -> Result<Value<'_>, Error> {
                panic!("callback exploded")
            })
            .unwrap();

        let class = env.find_class("usagi/Test").unwrap();
        let panic = class.static_method("panic", "()I").unwrap();
        let add = class.static_method("add", "(II)I").unwrap();

        let error = panic.call(None, &[]).map(|_| ()).unwrap_err();
        assert_eq!(error.operation(), "java exception");
        assert!(
            error.message().contains("callback exploded"),
            "{}",
            error.message()
        );

        match add.call(None, &[Value::Int(3), Value::Int(4)]).unwrap() {
            Output::Int(sum) => assert_eq!(sum, 7),
            _ => panic!("add should return an int"),
        }
    }

    #[test]
    fn registering_the_same_method_replaces_the_callback() {
        let vm = Vm::new().unwrap();
        let env = vm.env().unwrap();
        register_add(&env);

        env.define_class("usagi/Test")
            .unwrap()
            .register_static_method("add", "(II)I", |_env, args| match args {
                [Value::Int(a), Value::Int(b)] => Ok(Value::Int(a - b)),
                _ => Err(Error::new("callback", "unexpected arguments")),
            })
            .unwrap();

        let class = env.find_class("usagi/Test").unwrap();
        let add = class.static_method("add", "(II)I").unwrap();

        match add.call(None, &[Value::Int(3), Value::Int(4)]).unwrap() {
            Output::Int(difference) => assert_eq!(difference, -1),
            _ => panic!("add should return an int"),
        }
    }

    #[test]
    fn calls_work_after_an_exception() {
        let vm = Vm::new().unwrap();
        let env = vm.env().unwrap();
        register_add(&env);
        register_fail(&env);

        let class = env.find_class("usagi/Test").unwrap();
        let fail = class.static_method("fail", "()I").unwrap();
        let add = class.static_method("add", "(II)I").unwrap();

        assert!(fail.call(None, &[]).is_err());

        match add.call(None, &[Value::Int(3), Value::Int(4)]).unwrap() {
            Output::Int(sum) => assert_eq!(sum, 7),
            _ => panic!("add should return an int"),
        }
    }
}
