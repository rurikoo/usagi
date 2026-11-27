use std::{ffi::CString, sync::Arc};

use jni_sys::jvalue;
use usagi::{ArrayElement, ArrayKind, Error, Output, PinElement, Value, Vm};

#[test]
fn marshals_strings_classes_objects_and_null_across_callbacks() {
    let vm = Vm::new().unwrap();
    let env = vm.env().unwrap();
    let cls = env.define_class("usagi/Phase2Types").unwrap();

    cls.register_static_method(
        "greet",
        "(Ljava/lang/String;)Ljava/lang/String;",
        |env, args| match args {
            [Value::String(name)] => {
                let text = name.to_string_lossy()?;
                Ok(Value::String(env.new_string(&format!("hello, {text}"))?))
            }
            [Value::Null] => Ok(Value::Null),
            _ => Err(Error::new("callback", "unexpected args")),
        },
    )
    .unwrap();

    cls.register_static_method(
        "echoClass",
        "(Ljava/lang/Class;)Ljava/lang/Class;",
        |_env, args| match args {
            [Value::Class(c)] => Ok(Value::Class(c.clone())),
            [Value::Null] => Ok(Value::Null),
            _ => Err(Error::new("callback", "unexpected args")),
        },
    )
    .unwrap();

    cls.register_static_method(
        "echoObject",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        |_env, args| match args {
            [Value::Object(obj)] => Ok(Value::Object(obj.clone())),
            [Value::Null] => Ok(Value::Null),
            _ => Err(Error::new("callback", "unexpected args")),
        },
    )
    .unwrap();

    let greet = cls
        .static_method("greet", "(Ljava/lang/String;)Ljava/lang/String;")
        .unwrap();
    let input = env.new_string("usagi").unwrap();
    match greet.call(None, &[Value::String(input)]).unwrap() {
        Output::String(Some(s)) => assert_eq!(s.to_string_lossy().unwrap(), "hello, usagi"),
        _ => panic!("expected non-null String"),
    }
    assert!(matches!(
        greet.call(None, &[Value::Null]).unwrap(),
        Output::String(None)
    ));

    let echo_class = cls
        .static_method("echoClass", "(Ljava/lang/Class;)Ljava/lang/Class;")
        .unwrap();
    let str_cls = env.find_class("java/lang/String").unwrap();
    assert!(matches!(
        echo_class.call(None, &[Value::Class(str_cls)]).unwrap(),
        Output::Class(Some(_))
    ));
    assert!(matches!(
        echo_class.call(None, &[Value::Null]).unwrap(),
        Output::Class(None)
    ));

    let echo_obj = cls
        .static_method("echoObject", "(Ljava/lang/Object;)Ljava/lang/Object;")
        .unwrap();
    let packed_str = env.new_string("wrapped").unwrap();
    match echo_obj.call(None, &[Value::String(packed_str)]).unwrap() {
        Output::Object(Some(obj)) => {
            let s = obj.into_string().unwrap();
            assert_eq!(s.to_string_lossy().unwrap(), "wrapped");
        }
        _ => panic!("expected Object wrapping String"),
    }
}

fn roundtrip_primitive_slice<T: ArrayElement + PartialEq + std::fmt::Debug>(
    env: &usagi::Env<'_>,
    data: &[T],
) {
    let arr = env.new_primitive_array_from_slice(data).unwrap();
    assert_eq!(arr.kind(), T::KIND);
    assert_eq!(arr.len().unwrap(), data.len());
    assert_eq!(arr.is_empty().unwrap(), data.is_empty());
    assert_eq!(arr.to_vec::<T>().unwrap(), data);

    let empty_arr = env.new_primitive_array::<T>(0).unwrap();
    assert!(empty_arr.is_empty().unwrap());
    assert_eq!(empty_arr.to_vec::<T>().unwrap(), Vec::<T>::new());
}

fn roundtrip_pin<T: PinElement + PartialEq + std::fmt::Debug>(
    env: &usagi::Env<'_>,
    initial: &[T],
    updated: &[T],
) {
    assert_eq!(initial.len(), updated.len());
    let arr = env.new_primitive_array_from_slice(initial).unwrap();
    {
        let mut pinned = unsafe { arr.pin::<T>() }.unwrap();
        assert_eq!(&*pinned, initial);
        pinned.copy_from_slice(updated);
        pinned.commit().unwrap();
    }
    assert_eq!(arr.to_vec::<T>().unwrap(), updated);
}

#[test]
fn primitive_arrays_support_all_types_regions_bounds_and_pinning() {
    let vm = Vm::new().unwrap();
    let env = vm.env().unwrap();

    roundtrip_primitive_slice(&env, &[true, false, true]);
    roundtrip_primitive_slice(&env, &[-128i8, 0, 127]);
    roundtrip_primitive_slice(&env, &[0u16, 65, 0x1F98]);
    roundtrip_primitive_slice(&env, &[-32000i16, 0, 32000]);
    roundtrip_primitive_slice(&env, &[-1_000_000i32, 42, i32::MAX]);
    roundtrip_primitive_slice(&env, &[i64::MIN, 0, i64::MAX]);
    roundtrip_primitive_slice(&env, &[-1.25f32, 0.0, 3.5]);
    roundtrip_primitive_slice(&env, &[-2.5f64, 0.0, 9.75]);

    roundtrip_pin(&env, &[1i8, 2, 3], &[10, 20, 30]);
    roundtrip_pin(&env, &[1u16, 2, 3], &[4, 5, 6]);
    roundtrip_pin(&env, &[1i16, 2, 3], &[7, 8, 9]);
    roundtrip_pin(&env, &[10i32, 20, 30], &[100, 200, 300]);
    roundtrip_pin(&env, &[100i64, 200], &[300, 400]);
    roundtrip_pin(&env, &[1.0f32, 2.0], &[3.0, 4.0]);
    roundtrip_pin(&env, &[1.0f64, 2.0], &[5.0, 6.0]);

    // bounds and type checks
    let ints = env.new_primitive_array_from_slice(&[1i32, 2, 3]).unwrap();
    let mut buf = [0i32; 2];
    assert!(ints.get_region(2, &mut buf).is_err());
    assert!(ints.get_region(usize::MAX, &mut buf).is_err());
    assert!(ints.set_region(2, &[9i32, 10]).is_err());
    assert!(ints.to_vec::<i8>().is_err());
    assert!(unsafe { ints.pin::<i8>() }.is_err());

    // empty array pin works without UB
    let empty = env.new_primitive_array::<i32>(0).unwrap();
    let mut pinned = unsafe { empty.pin::<i32>() }.unwrap();
    assert_eq!(&*pinned, &[]);
    assert_eq!(&mut *pinned, &mut []);
    pinned.abort().unwrap();
}

#[test]
fn boolean_array_normalizes_non_canonical_raw_bytes() {
    let vm = Vm::new().unwrap();
    let env = vm.env().unwrap();

    let cls = env.define_class("usagi/BoolArr").unwrap();
    cls.register_static_method("check", "([Z)Z", |_env, args| match args {
        [Value::Array(arr)] => {
            let vals = arr.to_vec::<bool>()?;
            Ok(Value::Boolean(vals == vec![true, false, true]))
        }
        _ => Err(Error::new("callback", "expected boolean[]")),
    })
    .unwrap();

    let raw_env = env.raw_jni_env().as_ptr();
    let cls_name = CString::new("usagi/BoolArr").unwrap();
    let m_name = CString::new("check").unwrap();
    let m_sig = CString::new("([Z)Z").unwrap();

    unsafe {
        let v1_1 = (**raw_env).v1_1;
        let jcls = (v1_1.FindClass)(raw_env, cls_name.as_ptr());
        let mid = (v1_1.GetStaticMethodID)(raw_env, jcls, m_name.as_ptr(), m_sig.as_ptr());

        let jarr = (v1_1.NewBooleanArray)(raw_env, 3);
        let ptr = (v1_1.GetBooleanArrayElements)(raw_env, jarr, std::ptr::null_mut());
        // Write non-canonical byte 42 as a C caller might.
        let raw_bytes = ptr.cast::<u8>();
        *raw_bytes.add(0) = 42;
        *raw_bytes.add(1) = 0;
        *raw_bytes.add(2) = 255;
        (v1_1.ReleaseBooleanArrayElements)(raw_env, jarr, ptr, 0);

        let arg = [jvalue { l: jarr }];
        let ok = (v1_1.CallStaticBooleanMethodA)(raw_env, jcls, mid, arg.as_ptr());
        assert!(ok);

        (v1_1.DeleteLocalRef)(raw_env, jarr);
        (v1_1.DeleteLocalRef)(raw_env, jcls);
    }
}

#[test]
fn object_arrays_and_array_callbacks_work() {
    let vm = Vm::new().unwrap();
    let env = vm.env().unwrap();

    let str_cls = env.find_class("java/lang/String").unwrap();
    let init = env.new_string("default").unwrap();
    let arr = env
        .new_object_array(3, &str_cls, Some(init.as_object()))
        .unwrap();
    assert_eq!(arr.kind(), ArrayKind::Object);
    assert_eq!(arr.len().unwrap(), 3);

    let first = arr.get_object_element(0).unwrap().unwrap();
    assert_eq!(
        first.into_string().unwrap().to_string_lossy().unwrap(),
        "default"
    );

    let second = env.new_string("custom").unwrap();
    arr.set_object_element(1, Some(second.as_object())).unwrap();
    arr.set_object_element(2, None).unwrap();

    assert!(arr.get_object_element(3).is_err());
    assert!(arr.set_object_element(3, None).is_err());

    let cls = env.define_class("usagi/ArrayBridge").unwrap();
    cls.register_static_method(
        "joinStrings",
        "([Ljava/lang/String;)Ljava/lang/String;",
        |env, args| match args {
            [Value::Array(arr)] => {
                let mut parts = Vec::new();
                for i in 0..arr.len()? {
                    if let Some(obj) = arr.get_object_element(i)? {
                        parts.push(obj.into_string()?.to_string_lossy()?);
                    } else {
                        parts.push("<null>".to_owned());
                    }
                }
                Ok(Value::String(env.new_string(&parts.join(","))?))
            }
            _ => Err(Error::new("callback", "expected String[]")),
        },
    )
    .unwrap();

    cls.register_static_method("doubleBytes", "([B)[B", |env, args| match args {
        [Value::Array(arr)] => {
            let mut bytes = arr.to_vec::<i8>()?;
            for b in &mut bytes {
                *b = b.wrapping_mul(2);
            }
            Ok(Value::Array(env.new_primitive_array_from_slice(&bytes)?))
        }
        _ => Err(Error::new("callback", "expected byte[]")),
    })
    .unwrap();

    let join = cls
        .static_method("joinStrings", "([Ljava/lang/String;)Ljava/lang/String;")
        .unwrap();
    match join.call(None, &[Value::Array(arr)]).unwrap() {
        Output::String(Some(s)) => {
            assert_eq!(s.to_string_lossy().unwrap(), "default,custom,<null>")
        }
        _ => panic!("expected String"),
    }

    let double = cls.static_method("doubleBytes", "([B)[B").unwrap();
    let input_bytes = env.new_primitive_array_from_slice(&[1i8, 2, -3]).unwrap();
    match double.call(None, &[Value::Array(input_bytes)]).unwrap() {
        Output::Array(Some(out_arr)) => {
            assert_eq!(out_arr.to_vec::<i8>().unwrap(), vec![2, 4, -6]);
        }
        _ => panic!("expected byte[]"),
    }
}

#[test]
fn array_argument_mismatch_and_cross_vm_rejected() {
    let vm1 = Vm::new().unwrap();
    let vm2 = Vm::new().unwrap();
    let env1 = vm1.env().unwrap();
    let env2 = vm2.env().unwrap();

    let cls = env1.define_class("usagi/Mismatch").unwrap();
    cls.register_static_method("takeBytes", "([B)V", |_, _| Ok(Value::Void))
        .unwrap();
    let m = cls.static_method("takeBytes", "([B)V").unwrap();

    let ints_vm1 = env1.new_primitive_array::<i32>(2).unwrap();
    assert!(m.call(None, &[Value::Array(ints_vm1)]).is_err());

    let bytes_vm2 = env2.new_primitive_array::<i8>(2).unwrap();
    assert!(m.call(None, &[Value::Array(bytes_vm2)]).is_err());
}

#[test]
fn global_ref_array_roundtrips_across_threads() {
    let vm = Arc::new(Vm::new().unwrap());
    let worker_vm = Arc::clone(&vm);

    let global = std::thread::spawn(move || {
        let guard = worker_vm.attach_current_thread().unwrap();
        guard
            .new_primitive_array_from_slice(&[7i32, 8, 9])
            .unwrap()
            .into_global()
    })
    .join()
    .unwrap();

    let env = vm.env().unwrap();
    let arr = global.as_array(&env).unwrap();
    assert_eq!(arr.to_vec::<i32>().unwrap(), vec![7, 8, 9]);
}

#[test]
fn direct_byte_buffers_roundtrip_through_rust_and_jni() {
    let vm = Vm::new().unwrap();
    let env = vm.env().unwrap();

    let mut backing = vec![1u8, 2, 3, 4, 5];
    let buf = unsafe { env.new_direct_byte_buffer(backing.as_mut_ptr(), backing.len()) }.unwrap();

    assert_eq!(
        env.get_direct_buffer_address(&buf).unwrap(),
        backing.as_mut_ptr()
    );
    assert_eq!(env.get_direct_buffer_capacity(&buf).unwrap(), 5);

    let cls = env.define_class("usagi/DirectBuf").unwrap();
    cls.register_static_method(
        "flipFirst",
        "(Ljava/nio/ByteBuffer;)Ljava/nio/ByteBuffer;",
        |env, args| match args {
            [Value::Object(bb)] => {
                let ptr = env.get_direct_buffer_address(bb)?;
                let cap = env.get_direct_buffer_capacity(bb)?;
                assert_eq!(cap, 5);
                unsafe {
                    *ptr = 99;
                }
                Ok(Value::Object(bb.clone()))
            }
            _ => Err(Error::new("callback", "expected ByteBuffer")),
        },
    )
    .unwrap();

    let flip = cls
        .static_method("flipFirst", "(Ljava/nio/ByteBuffer;)Ljava/nio/ByteBuffer;")
        .unwrap();
    match flip.call(None, &[Value::Object(buf)]).unwrap() {
        Output::Object(Some(returned)) => {
            assert_eq!(
                env.get_direct_buffer_address(&returned).unwrap(),
                backing.as_mut_ptr()
            );
            assert_eq!(env.get_direct_buffer_capacity(&returned).unwrap(), 5);
        }
        _ => panic!("expected ByteBuffer object"),
    }
    assert_eq!(backing[0], 99);

    // Non-ByteBuffer objects are rejected cleanly
    let not_bb = env.new_string("not a buffer").unwrap();
    assert!(env.get_direct_buffer_address(not_bb.as_object()).is_err());
    assert!(env.get_direct_buffer_capacity(not_bb.as_object()).is_err());

    // Null address is rejected
    assert!(unsafe { env.new_direct_byte_buffer(std::ptr::null_mut(), 4) }.is_err());
}
