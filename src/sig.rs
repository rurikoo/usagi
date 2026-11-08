use crate::error::Error;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Boolean,
    Byte,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
    Void,
    Object(String),   // class name, like "java/lang/String"
    Array(Box<Type>), // element type
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    text: String, // the original text, like "(ILjava/lang/String;)Z"
    args: Vec<Type>,
    ret: Type,
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    /// reads one complete type and leaves the cursor right after it
    fn parse_type(&mut self, depth: usize) -> Result<Type, Error> {
        match self.peek() {
            Some(b'V') => {
                self.bump();
                Ok(Type::Void)
            }
            Some(b'Z') => {
                self.bump();
                Ok(Type::Boolean)
            }
            Some(b'I') => {
                self.bump();
                Ok(Type::Int)
            }
            Some(b'J') => {
                self.bump();
                Ok(Type::Long)
            }
            Some(b'D') => {
                self.bump();
                Ok(Type::Double)
            }
            Some(b'F') => {
                self.bump();
                Ok(Type::Float)
            }
            Some(b'B') => {
                self.bump();
                Ok(Type::Byte)
            }
            Some(b'C') => {
                self.bump();
                Ok(Type::Char)
            }
            Some(b'S') => {
                self.bump();
                Ok(Type::Short)
            }
            Some(b'L') => {
                self.bump();
                let start = self.pos;

                let offset = self.bytes[start..]
                    .iter()
                    .position(|&b| b == b';')
                    .ok_or_else(|| err(format!("missing ';' in class name at position {start}")))?;

                if offset == 0 {
                    return Err(err(format!("empty class name at position {start}")));
                }

                let name = std::str::from_utf8(&self.bytes[start..start + offset])
                    .map_err(|_| err("class name is not valid utf-8"))?
                    .to_owned();

                // move past the ';'
                self.pos = start + offset + 1;

                Ok(Type::Object(name))
            }
            Some(b'[') => {
                if depth >= 255 {
                    return Err(err("array nesting exceeds 255 dimensions"));
                }
                self.bump();
                let element = self.parse_type(depth + 1)?;

                if element == Type::Void {
                    return Err(err(format!("void array at position {}", self.pos)));
                }
                Ok(Type::Array(Box::new(element)))
            }
            Some(other) => Err(err(format!(
                "unknown type code {:?} at position {}",
                other as char, self.pos
            ))),
            None => Err(err("unexpected end of signature")),
        }
    }
}

fn err(message: impl Into<String>) -> Error {
    Error::new("signature parse", message)
}

impl Signature {
    pub fn args(&self) -> &[Type] {
        &self.args
    }

    pub fn ret(&self) -> &Type {
        &self.ret
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            pos: 0,
        };

        if parser.peek() != Some(b'(') {
            return Err(err("missing opening parenthesis"));
        }
        parser.bump();

        let mut args = Vec::new();
        loop {
            match parser.peek() {
                Some(b')') => {
                    parser.bump();
                    break;
                }
                Some(_) => {
                    let ty = parser.parse_type(0)?;
                    if ty == Type::Void {
                        return Err(err("void is not allowed as an argument"));
                    }
                    args.push(ty);
                }
                None => return Err(err("missing closing parenthesis")),
            }
        }

        let ret = parser.parse_type(0)?;

        if parser.pos != parser.bytes.len() {
            return Err(err(format!(
                "unexpected trailing data at position {}",
                parser.pos
            )));
        }

        Ok(Self {
            text: text.to_owned(),
            args,
            ret,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(name: &str) -> Type {
        Type::Object(name.to_owned())
    }

    fn arr(element: Type) -> Type {
        Type::Array(Box::new(element))
    }

    fn parse_ok(text: &str) -> Signature {
        Signature::parse(text).unwrap_or_else(|e| panic!("{text:?} should parse: {e}"))
    }

    fn parse_err(text: &str, needle: &str) {
        let error = Signature::parse(text).expect_err(&format!("{text:?} should not parse"));
        assert!(
            error.message().contains(needle),
            "{text:?}: expected message containing {needle:?}, got {:?}",
            error.message()
        );
    }

    #[test]
    fn parses_no_arguments() {
        let sig = parse_ok("()V");
        assert_eq!(sig.args, vec![]);
        assert_eq!(sig.ret, Type::Void);
    }

    #[test]
    fn parses_every_primitive_in_order() {
        let sig = parse_ok("(ZBCSIJFD)V");
        assert_eq!(
            sig.args,
            vec![
                Type::Boolean,
                Type::Byte,
                Type::Char,
                Type::Short,
                Type::Int,
                Type::Long,
                Type::Float,
                Type::Double,
            ]
        );
    }

    #[test]
    fn parses_primitive_return_types() {
        let cases = [
            ("()Z", Type::Boolean),
            ("()B", Type::Byte),
            ("()C", Type::Char),
            ("()S", Type::Short),
            ("()I", Type::Int),
            ("()J", Type::Long),
            ("()F", Type::Float),
            ("()D", Type::Double),
            ("()V", Type::Void),
        ];

        for (text, expected) in cases {
            assert_eq!(parse_ok(text).ret, expected, "{text}");
        }
    }

    #[test]
    fn parses_object_argument() {
        let sig = parse_ok("(Ljava/lang/String;)V");
        assert_eq!(sig.args, vec![obj("java/lang/String")]);
    }

    #[test]
    fn continues_after_object_type() {
        // proves the cursor moved past the ';'
        let sig = parse_ok("(Ljava/lang/String;I)Z");
        assert_eq!(sig.args, vec![obj("java/lang/String"), Type::Int]);
        assert_eq!(sig.ret, Type::Boolean);
    }

    #[test]
    fn parses_adjacent_objects() {
        let sig = parse_ok("(La/B;Lc/D;)V");
        assert_eq!(sig.args, vec![obj("a/B"), obj("c/D")]);
    }

    #[test]
    fn class_names_containing_type_letters_are_not_misread() {
        // 'L', 'J', 'I' and so on inside the name must not count as types
        let sig = parse_ok("(Ljava/lang/Long;)Ljava/lang/Integer;");
        assert_eq!(sig.args, vec![obj("java/lang/Long")]);
        assert_eq!(sig.ret, obj("java/lang/Integer"));
    }

    #[test]
    fn parses_nested_class_names() {
        let sig = parse_ok("(Lcom/example/Outer$Inner;)V");
        assert_eq!(sig.args, vec![obj("com/example/Outer$Inner")]);
    }

    #[test]
    fn parses_non_ascii_class_names() {
        let sig = parse_ok("(Lja/vä;)V");
        assert_eq!(sig.args, vec![obj("ja/vä")]);
    }

    #[test]
    fn parses_arrays() {
        let sig = parse_ok("([B[Ljava/lang/String;)V");
        assert_eq!(
            sig.args,
            vec![arr(Type::Byte), arr(obj("java/lang/String"))]
        );
    }

    #[test]
    fn parses_nested_arrays() {
        let sig = parse_ok("([[I)V");
        assert_eq!(sig.args, vec![arr(arr(Type::Int))]);
    }

    #[test]
    fn parses_deeply_nested_arrays() {
        let text = format!("({}I)V", "[".repeat(100));
        let sig = parse_ok(&text);

        let mut expected = Type::Int;
        for _ in 0..100 {
            expected = arr(expected);
        }
        assert_eq!(sig.args, vec![expected]);
    }

    #[test]
    fn parses_array_return_type() {
        let sig = parse_ok("()[Ljava/lang/String;");
        assert_eq!(sig.ret, arr(obj("java/lang/String")));
    }

    #[test]
    fn keeps_the_original_text() {
        let text = "(ILjava/lang/String;)Z";
        assert_eq!(parse_ok(text).text, text);
    }

    #[test]
    fn rejects_empty_input() {
        parse_err("", "opening parenthesis");
    }

    #[test]
    fn rejects_missing_opening_parenthesis() {
        parse_err("I", "opening parenthesis");
        parse_err("I)V", "opening parenthesis");
    }

    #[test]
    fn rejects_missing_closing_parenthesis() {
        parse_err("(", "closing parenthesis");
        parse_err("(I", "closing parenthesis");
    }

    #[test]
    fn rejects_missing_return_type() {
        parse_err("()", "unexpected end");
        parse_err("(I)", "unexpected end");
    }

    #[test]
    fn rejects_unknown_type_codes() {
        parse_err("(Q)V", "unknown type code");
        parse_err("()Q", "unknown type code");
        parse_err("(i)V", "unknown type code"); // codes are case sensitive
    }

    #[test]
    fn rejects_trailing_data() {
        parse_err("()VV", "trailing");
        parse_err("(I)IX", "trailing");
    }

    #[test]
    fn rejects_object_without_semicolon() {
        parse_err("(Ljava/lang/String)V", "missing ';'");
        parse_err("(L", "missing ';'");
        parse_err("()L", "missing ';'");
    }

    #[test]
    fn rejects_empty_class_name() {
        parse_err("(L;)V", "empty class name");
    }

    #[test]
    fn rejects_array_without_element() {
        parse_err("([)V", "unknown type code");
        parse_err("()[", "unexpected end");
    }

    #[test]
    fn rejects_void_array() {
        parse_err("()[V", "void");
        parse_err("([V)V", "void");
    }

    #[test]
    fn rejects_void_argument() {
        parse_err("(V)V", "void");
        parse_err("(IV)V", "void");
    }

    #[test]
    fn accepts_maximum_array_depth() {
        parse_ok(&format!("({}I)V", "[".repeat(255)));
    }

    #[test]
    fn rejects_excessive_array_depth() {
        parse_err(&format!("({}I)V", "[".repeat(256)), "nesting");
    }
}
