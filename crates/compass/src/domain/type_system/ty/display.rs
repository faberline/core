use std::fmt;

use crate::domain::type_system::ty::{LiteralValue, Type};

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Never => write!(f, "Never"),
            Type::None => write!(f, "None"),
            Type::Bool => write!(f, "bool"),
            Type::Int => write!(f, "int"),
            Type::Float => write!(f, "float"),
            Type::Str => write!(f, "str"),
            Type::Bytes => write!(f, "bytes"),

            Type::List(elem) => write!(f, "list[{}]", elem),
            Type::Dict(k, v) => write!(f, "dict[{}, {}]", k, v),
            Type::Set(elem) => write!(f, "set[{}]", elem),
            Type::Tuple(elems) => {
                write!(f, "tuple[")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, "]")
            }

            Type::Optional(inner) => write!(f, "{} | None", inner),
            Type::Union(types) => {
                for (i, ty) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", ty)?;
                }
                Ok(())
            }
            Type::Intersection(types) => {
                for (i, ty) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, " & ")?;
                    }
                    write!(f, "{}", ty)?;
                }
                Ok(())
            }

            Type::Callable { params, ret } => {
                write!(f, "(")?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", param.ty)?;
                }
                write!(f, ") -> {}", ret)
            }

            Type::Instance {
                name, type_args, ..
            } => {
                write!(f, "{}", name)?;
                if !type_args.is_empty() {
                    write!(f, "[")?;
                    for (i, arg) in type_args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, "]")?;
                }
                Ok(())
            }
            Type::ClassType { name, .. } => write!(f, "type[{}]", name),

            Type::Protocol { name, members, .. } => {
                write!(f, "Protocol[{}]", name)?;
                if !members.is_empty() {
                    write!(f, "{{")?;
                    for (i, (member_name, _)) in members.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", member_name)?;
                    }
                    write!(f, "}}")?;
                }
                Ok(())
            }

            Type::TypedDict { name, fields, .. } => {
                write!(f, "TypedDict[{}]", name)?;
                if !fields.is_empty() {
                    write!(f, "{{")?;
                    for (i, (field_name, field_ty, required)) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        if *required {
                            write!(f, "{}: {}", field_name, field_ty)?;
                        } else {
                            write!(f, "{}?: {}", field_name, field_ty)?;
                        }
                    }
                    write!(f, "}}")?;
                }
                Ok(())
            }

            Type::TypeVar { name, .. } => write!(f, "{}", name),

            Type::Any => write!(f, "Any"),
            Type::Unknown => write!(f, "Unknown"),
            Type::Literal(lit) => match lit {
                LiteralValue::Int(n) => write!(f, "Literal[{}]", n),
                LiteralValue::Float(n) => write!(f, "Literal[{}]", n),
                LiteralValue::Str(s) => write!(f, "Literal[\"{}\"]", s),
                LiteralValue::Bool(b) => write!(f, "Literal[{}]", b),
                LiteralValue::None => write!(f, "Literal[None]"),
            },
            Type::SelfType { class_name } => {
                if let Some(name) = class_name {
                    write!(f, "Self[{}]", name)
                } else {
                    write!(f, "Self")
                }
            }
            Type::LiteralString => write!(f, "LiteralString"),
            Type::Final(inner) => write!(f, "Final[{}]", inner),
            Type::Annotated { inner, metadata } => {
                write!(f, "Annotated[{}", inner)?;
                for m in metadata {
                    write!(f, ", {}", m)?;
                }
                write!(f, "]")
            }
            Type::Overloaded { signatures } => {
                write!(f, "Overloaded[")?;
                for (i, sig) in signatures.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", sig)?;
                }
                write!(f, "]")
            }
            Type::ParamSpec { name, .. } => write!(f, "ParamSpec[{}]", name),
            Type::Concatenate { params, param_spec } => {
                write!(f, "Concatenate[")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                if !params.is_empty() {
                    write!(f, ", ")?;
                }
                write!(f, "{}]", param_spec)
            }
            Type::TypeVarTuple { name, .. } => write!(f, "TypeVarTuple[{}]", name),
            Type::Unpack(inner) => write!(f, "*{}", inner),
            Type::TypeGuard(inner) => write!(f, "TypeGuard[{}]", inner),
            Type::TypeIs(inner) => write!(f, "TypeIs[{}]", inner),

            Type::Error => write!(f, "<error>"),
        }
    }
}
