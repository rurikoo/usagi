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
        java_string.to_string_lossy().expect("sstring conversion"),
        original
    );
}

#[test]
fn rejects_invalid_class_name() {
    let vm = Vm::new().expect("VM creation should succeed");

    assert!(vm.find_class("java\0/lang/String").is_err());
}
