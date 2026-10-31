use usagi::Vm;

fn main() {
    let vm = Vm::new().unwrap();
    let string = vm.new_string("hi").unwrap();
    drop(vm);
    let _ = string.to_string_lossy();
}
