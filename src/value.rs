use crate::{error::Error, sig::Type};

#[derive(Debug, Clone, Copy)]
pub enum Value {
    Boolean(bool),
    Byte(i8),
    Char(u16),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    /// valid for any object or array param
    Null,
}

impl Value {
    pub fn name(&self) -> &'static str {
        match self {
            Value::Boolean(_) => "boolean",
            Value::Byte(_) => "byte",
            Value::Char(_) => "char",
            Value::Short(_) => "short",
            Value::Int(_) => "int",
            Value::Long(_) => "long",
            Value::Float(_) => "float",
            Value::Double(_) => "double",
            Value::Null => "null",
        }
    }
}

impl Type {
    pub fn name(&self) -> &'static str {
        match self {
            Type::Boolean => "boolean",
            Type::Byte => "byte",
            Type::Char => "char",
            Type::Short => "short",
            Type::Int => "int",
            Type::Long => "long",
            Type::Float => "float",
            Type::Double => "double",
            Type::Void => "void",
            Type::Object(_) => "object",
            Type::Array(_) => "array",
        }
    }
}

/// returns true when value passed where expected is declared
fn fits(value: &Value, expected: &Type) -> bool {
    matches!(
        (value, expected),
        (Value::Boolean(_), Type::Boolean)
            | (Value::Byte(_), Type::Byte)
            | (Value::Char(_), Type::Char)
            | (Value::Short(_), Type::Short)
            | (Value::Int(_), Type::Int)
            | (Value::Long(_), Type::Long)
            | (Value::Float(_), Type::Float)
            | (Value::Double(_), Type::Double)
            | (Value::Null, Type::Object(_) | Type::Array(_))
    )
}

/// checks that args can be passed to a method with params expected
pub fn check_args(args: &[Value], expected: &[Type]) -> Result<(), Error> {
    if args.len() != expected.len() {
        return Err(Error::new(
            "argument check",
            format!(
                "expected {} argument(s), got {}",
                expected.len(),
                args.len()
            ),
        ));
    }

    for (index, (value, ty)) in args.iter().zip(expected).enumerate() {
        if !fits(value, ty) {
            return Err(Error::new(
                "argument check",
                format!(
                    "argument {index}: expected {}, got {}",
                    ty.name(),
                    value.name()
                ),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sig::Signature;

    fn obj(name: &str) -> Type {
        Type::Object(name.to_owned())
    }

    fn arr(element: Type) -> Type {
        Type::Array(Box::new(element))
    }

    #[test]
    fn every_primitive_fits_its_own_type() {
        assert!(fits(&Value::Boolean(true), &Type::Boolean));
        assert!(fits(&Value::Byte(1), &Type::Byte));
        assert!(fits(&Value::Char(1), &Type::Char));
        assert!(fits(&Value::Short(1), &Type::Short));
        assert!(fits(&Value::Int(1), &Type::Int));
        assert!(fits(&Value::Long(1), &Type::Long));
        assert!(fits(&Value::Float(1.0), &Type::Float));
        assert!(fits(&Value::Double(1.0), &Type::Double));
    }

    #[test]
    fn no_widening_between_primitives() {
        assert!(!fits(&Value::Int(1), &Type::Long));
        assert!(!fits(&Value::Byte(1), &Type::Int));
        assert!(!fits(&Value::Boolean(true), &Type::Int));
        assert!(!fits(&Value::Float(1.0), &Type::Double));
    }

    #[test]
    fn primitives_never_fit_references() {
        assert!(!fits(&Value::Int(1), &obj("java/lang/Integer")));
        assert!(!fits(&Value::Int(1), &arr(Type::Int)));
    }

    #[test]
    fn null_fits_references_only() {
        assert!(fits(&Value::Null, &obj("java/lang/String")));
        assert!(fits(&Value::Null, &arr(Type::Byte)));
        assert!(!fits(&Value::Null, &Type::Int));
        assert!(!fits(&Value::Null, &Type::Void));
    }

    #[test]
    fn void_never_fits() {
        assert!(!fits(&Value::Int(1), &Type::Void));
    }

    #[test]
    fn empty_signature_accepts_no_values() {
        assert!(check_args(&[], &[]).is_ok());
    }

    #[test]
    fn too_few_values_reports_both_counts() {
        let error = check_args(&[Value::Int(3)], &[Type::Int, Type::Int]).unwrap_err();
        assert!(
            error.message().contains("expected 2"),
            "{}",
            error.message()
        );
        assert!(error.message().contains("got 1"), "{}", error.message());
    }

    #[test]
    fn too_many_values_reports_both_counts() {
        let error = check_args(&[Value::Int(3), Value::Int(4)], &[Type::Int]).unwrap_err();
        assert!(
            error.message().contains("expected 1"),
            "{}",
            error.message()
        );
        assert!(error.message().contains("got 2"), "{}", error.message());
    }

    #[test]
    fn mismatch_names_the_argument_index() {
        let error =
            check_args(&[Value::Int(3), Value::Long(4)], &[Type::Int, Type::Int]).unwrap_err();
        assert!(
            error.message().starts_with("argument 1:"),
            "{}",
            error.message()
        );
        assert!(
            error.message().contains("expected int"),
            "{}",
            error.message()
        );
        assert!(error.message().contains("got long"), "{}", error.message());
    }

    #[test]
    fn matching_add_signature_passes() {
        let sig = Signature::parse("(II)I").unwrap();
        assert!(check_args(&[Value::Int(3), Value::Int(4)], sig.args()).is_ok());
    }
}
