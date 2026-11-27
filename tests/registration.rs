use std::{
    ffi::CString,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
};

use jni_sys::jvalue;
use usagi::{Error, Output, Value, Vm};

#[test]
fn registers_and_calls_a_static_method() {
    let vm = Vm::new().expect("VM creation should succeed");
    let env = vm.env().expect("JNI env");
    let class = env.define_class("usagi/Test").expect("class definition");

    class
        .register_static_method("add", "(II)I", |_env, args| match args {
            [Value::Int(a), Value::Int(b)] => Ok(Value::Int(a + b)),
            _ => Err(Error::new("callback", "unexpected arguments")),
        })
        .expect("add(II)I should register");

    let add = class
        .static_method("add", "(II)I")
        .expect("add(II)I should be found");

    match add
        .call(None, &[Value::Int(3), Value::Int(4)])
        .expect("call")
    {
        Output::Int(sum) => assert_eq!(sum, 7),
        _ => panic!("add should return an int"),
    }
}

#[test]
fn registers_and_calls_void_static_method_with_side_effects() {
    let vm = Vm::new().expect("VM creation should succeed");
    let env = vm.env().expect("JNI env");
    let class = env.define_class("usagi/SideEffect").expect("class");

    let counter = Arc::new(AtomicI32::new(0));
    let counter_cb = Arc::clone(&counter);

    class
        .register_static_method("inc", "(I)V", move |_env, args| match args {
            [Value::Int(delta)] => {
                counter_cb.fetch_add(*delta, Ordering::SeqCst);
                Ok(Value::Void)
            }
            _ => Err(Error::new("callback", "unexpected arguments")),
        })
        .expect("inc(I)V should register");

    let inc = class.static_method("inc", "(I)V").expect("lookup");
    assert!(matches!(
        inc.call(None, &[Value::Int(5)]).expect("call"),
        Output::Void
    ));
    assert_eq!(counter.load(Ordering::SeqCst), 5);
}

#[test]
fn converts_every_primitive_argument() {
    let vm = Vm::new().expect("VM creation should succeed");
    let env = vm.env().expect("JNI env");
    let class = env.define_class("usagi/Mixed").expect("class definition");

    class
        .register_static_method("mix", "(ZBCSIJFD)D", |_env, args| match args {
            [
                Value::Boolean(z),
                Value::Byte(b),
                Value::Char(c),
                Value::Short(s),
                Value::Int(i),
                Value::Long(j),
                Value::Float(f),
                Value::Double(d),
            ] => {
                assert!(*z);
                assert_eq!(*b, -1);
                assert_eq!(*c, 2);
                assert_eq!(*s, -3);
                assert_eq!(*i, 4);
                assert_eq!(*j, -5);
                assert_eq!(*f, 6.5);
                assert_eq!(*d, -7.5);
                Ok(Value::Double(1.5))
            }
            _ => Err(Error::new("callback", "unexpected arguments")),
        })
        .expect("mix signature should register");

    let mix = class
        .static_method("mix", "(ZBCSIJFD)D")
        .expect("mix should be found");

    let args = [
        Value::Boolean(true),
        Value::Byte(-1),
        Value::Char(2),
        Value::Short(-3),
        Value::Int(4),
        Value::Long(-5),
        Value::Float(6.5),
        Value::Double(-7.5),
    ];

    match mix.call(None, &args).expect("call") {
        Output::Double(value) => assert_eq!(value, 1.5),
        _ => panic!("mix should return a double"),
    }
}

#[test]
fn callback_error_message_reaches_the_caller() {
    let vm = Vm::new().expect("VM creation should succeed");
    let env = vm.env().expect("JNI env");
    let class = env.define_class("usagi/Test").expect("class definition");

    class
        .register_static_method("fail", "()I", |_, _| {
            Err(Error::new("callback", "callback says no"))
        })
        .expect("fail()I should register");

    let fail = class
        .static_method("fail", "()I")
        .expect("fail()I should be found");

    let error = fail.call(None, &[]).map(|_| ()).unwrap_err();

    assert_eq!(error.operation(), "java exception");
    assert!(
        error.message().contains("callback says no"),
        "{}",
        error.message()
    );
}

#[test]
fn raw_jni_caller_gets_pending_exception_without_cpp_unwind() {
    let vm = Vm::new().unwrap();
    let env = vm.env().unwrap();
    let class = env.define_class("usagi/RawJni").unwrap();

    class
        .register_static_method("boom", "()I", |_, _| {
            Err(Error::new("callback", "boom from rust"))
        })
        .unwrap();

    class
        .register_static_method("isTrue", "(Z)Z", |_env, args| match args {
            [Value::Boolean(v)] => Ok(Value::Boolean(*v)),
            _ => Err(Error::new("callback", "unexpected arguments")),
        })
        .unwrap();

    let raw_env = env.raw_jni_env().as_ptr();
    let cls_name = CString::new("usagi/RawJni").unwrap();
    let boom_name = CString::new("boom").unwrap();
    let boom_sig = CString::new("()I").unwrap();
    let bool_name = CString::new("isTrue").unwrap();
    let bool_sig = CString::new("(Z)Z").unwrap();

    unsafe {
        let v1_1 = (**raw_env).v1_1;
        let v1_2 = (**raw_env).v1_2;

        let cls = (v1_1.FindClass)(raw_env, cls_name.as_ptr());
        assert!(!cls.is_null());

        let boom_mid =
            (v1_1.GetStaticMethodID)(raw_env, cls, boom_name.as_ptr(), boom_sig.as_ptr());
        assert!(!boom_mid.is_null());

        // must return normally with a pending JNI exception
        let _ = (v1_1.CallStaticIntMethodA)(raw_env, cls, boom_mid, std::ptr::null());
        assert!((v1_2.ExceptionCheck)(raw_env));
        (v1_1.ExceptionClear)(raw_env);
        assert!(!(v1_2.ExceptionCheck)(raw_env));

        let bool_mid =
            (v1_1.GetStaticMethodID)(raw_env, cls, bool_name.as_ptr(), bool_sig.as_ptr());
        assert!(!bool_mid.is_null());
        let non_canonical = [jvalue { b: 42 }];
        let result =
            (v1_1.CallStaticBooleanMethodA)(raw_env, cls, bool_mid, non_canonical.as_ptr());
        assert!(result);

        (v1_1.DeleteLocalRef)(raw_env, cls);
    }
}

#[test]
fn panicking_drop_in_replaced_callback_does_not_abort() {
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("drop panic inside callback holder");
        }
    }

    let vm = Vm::new().unwrap();
    let env = vm.env().unwrap();
    let class = env.define_class("usagi/DropTest").unwrap();

    let bomb = Bomb;
    class
        .register_static_method("f", "()I", move |_, _| {
            let _ = &bomb;
            Ok(Value::Int(1))
        })
        .unwrap();

    class
        .register_static_method("f", "()I", |_, _| Ok(Value::Int(2)))
        .unwrap();

    let f = class.static_method("f", "()I").unwrap();
    assert!(matches!(f.call(None, &[]).unwrap(), Output::Int(2)));
}

#[test]
fn multi_threaded_calls_via_attach_guard() {
    let vm = Arc::new(Vm::new().unwrap());
    {
        let env = vm.env().unwrap();
        let class = env.define_class("usagi/Threaded").unwrap();
        class
            .register_static_method("mul", "(II)I", |_env, args| match args {
                [Value::Int(a), Value::Int(b)] => Ok(Value::Int(a * b)),
                _ => Err(Error::new("callback", "unexpected arguments")),
            })
            .unwrap();
    }

    let handles: Vec<_> = (1..=4)
        .map(|i| {
            let vm = Arc::clone(&vm);
            std::thread::spawn(move || {
                let guard = vm.attach_current_thread().unwrap();
                let class = guard.find_class("usagi/Threaded").unwrap();
                let mul = class.static_method("mul", "(II)I").unwrap();
                match mul.call(None, &[Value::Int(i), Value::Int(10)]).unwrap() {
                    Output::Int(v) => assert_eq!(v, i * 10),
                    _ => panic!("expected int"),
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}
