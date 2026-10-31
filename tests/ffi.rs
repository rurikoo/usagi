use usagi::Vm;

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
