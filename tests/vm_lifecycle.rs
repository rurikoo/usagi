use usagi::Vm;

#[test]
fn creates_and_destroys_vm() {
    let vm = Vm::new().expect("VM creation should succeed");

    drop(vm);
}
