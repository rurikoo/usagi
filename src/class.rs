use std::{ffi::c_int, ptr::NonNull};

use crate::{
    callback,
    error::Error,
    ffi,
    method::Method,
    object::Object,
    sig::{Signature, Type},
    value::Value,
    vm::encode_modified_utf8,
};

pub struct Class<'vm> {
    object: Object<'vm>,
}

impl<'vm> Class<'vm> {
    pub(crate) fn from_object(object: Object<'vm>) -> Self {
        Self { object }
    }

    /// the underlying object handle
    pub fn as_object(&self) -> &Object<'vm> {
        &self.object
    }

    pub fn method(&self, name: &str, signature: &str) -> Result<Method<'_>, Error> {
        self.lookup(name, signature, false)
    }

    pub fn static_method(&self, name: &str, signature: &str) -> Result<Method<'_>, Error> {
        self.lookup(name, signature, true)
    }

    /// registers or replaces a static method backed by a Rust callback
    pub fn register_static_method<F>(
        &self,
        name: &str,
        signature: &str,
        handler: F,
    ) -> Result<(), Error>
    where
        F: Fn(&[Value]) -> Result<Value, Error> + Send + Sync + 'static,
    {
        let signature = Signature::parse(signature)?;

        if name.is_empty() || name.contains('\0') {
            return Err(Error::new(
                "method registration",
                "method name must be nonempty and contain no NUL",
            ));
        }

        for ty in signature.args() {
            if !is_primitive(ty) {
                return Err(Error::new(
                    "method registration",
                    format!("argument type {} is not supported yet", ty.name()),
                ));
            }
        }

        if !is_primitive(signature.ret()) {
            return Err(Error::new(
                "method registration",
                format!(
                    "return type {} is not supported yet",
                    signature.ret().name()
                ),
            ));
        }

        let name = encode_modified_utf8(name);
        let signature_text = encode_modified_utf8(signature.as_str());
        let user_data = callback::into_raw(signature, Box::new(handler));

        let status = unsafe {
            ffi::usagi_class_register_method(
                self.object.raw.as_ptr(),
                name.as_ptr().cast(),
                signature_text.as_ptr().cast(),
                c_int::from(true),
                user_data,
                Some(callback::trampoline),
                Some(callback::drop_holder),
            )
        };

        if status != ffi::USAGI_OK {
            return Err(Error::from_native(
                "method registration",
                "failed to register method",
            ));
        }

        Ok(())
    }

    fn lookup(&self, name: &str, signature: &str, is_static: bool) -> Result<Method<'_>, Error> {
        // parse first so invalid text never reaches C++!!
        let signature = Signature::parse(signature)?;

        if name.is_empty() || name.contains('\0') {
            return Err(Error::new(
                "method lookup",
                "method name must be nonempty and contain no NUL",
            ));
        }

        let name = encode_modified_utf8(name);
        let signature_text = encode_modified_utf8(signature.as_str());

        let raw = unsafe {
            ffi::usagi_class_get_method(
                self.object.raw.as_ptr(),
                name.as_ptr().cast(),
                signature_text.as_ptr().cast(),
                c_int::from(is_static),
            )
        };

        let id = NonNull::new(raw)
            .ok_or_else(|| Error::from_native("method lookup", "failed to find method"))?;

        Ok(Method {
            id,
            signature,
            is_static,
            class: self.object.raw,
            vm: self.object.vm(),
        })
    }
}

fn is_primitive(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Boolean
            | Type::Byte
            | Type::Char
            | Type::Short
            | Type::Int
            | Type::Long
            | Type::Float
            | Type::Double
    )
}

#[cfg(test)]
mod tests {
    use crate::{Error, Value, Vm};

    #[test]
    fn registers_a_static_method() {
        let vm = Vm::new().expect("VM creation should succeed");

        let class = vm
            .define_class("usagi/Test")
            .expect("usagi/Test should exist");

        class
            .register_static_method("add", "(II)I", |args| match args {
                [Value::Int(a), Value::Int(b)] => Ok(Value::Int(a + b)),
                _ => Err(Error::new("callback", "unexpected arguments")),
            })
            .expect("add(II)I should register");

        class
            .static_method("add", "(II)I")
            .expect("add(II)I should be found after registration");
    }

    #[test]
    fn rejects_non_primitive_signatures() {
        let vm = Vm::new().unwrap();
        let class = vm.define_class("usagi/Test").unwrap();

        fn handler(_: &[Value]) -> Result<Value, Error> {
            Ok(Value::Int(0))
        }

        assert!(
            class
                .register_static_method("a", "(Ljava/lang/String;)I", handler)
                .is_err()
        );
        assert!(class.register_static_method("b", "()[I", handler).is_err());
        assert!(class.register_static_method("c", "()V", handler).is_err());
    }
}
