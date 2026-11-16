use usagi::{Error, Output, Value, Vm};

#[test]
fn registers_and_calls_a_static_method() {
    let vm = Vm::new().expect("VM creation should succeed");
    let class = vm.define_class("usagi/Test").expect("class definition");

    class
        .register_static_method("add", "(II)I", |args| match args {
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
fn converts_every_primitive_argument() {
    let vm = Vm::new().expect("VM creation should succeed");
    let class = vm.define_class("usagi/Mixed").expect("class definition");

    class
        .register_static_method("mix", "(ZBCSIJFD)D", |args| match args {
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
    let class = vm.define_class("usagi/Test").expect("class definition");

    class
        .register_static_method("fail", "()I", |_| {
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
