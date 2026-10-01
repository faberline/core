use crate::domain::type_codegen::generator::CodeGenerator;
use crate::domain::type_codegen::request::CodeGenRequest;
use crate::domain::type_codegen::result::CodeGenResult;
use crate::domain::type_system::class_info::ClassInfo;
use crate::domain::type_system::ty::Type;

impl CodeGenerator {
    /// Generate type stub (.pyi file).
    pub(super) fn generate_type_stub(&self, request: &CodeGenRequest) -> CodeGenResult {
        use crate::domain::type_system::ty::Type;

        // Try to get type information from context
        let type_info = self
            .type_context
            .get_binding(&request.file, &request.symbol);

        let stub = match type_info.map(|b| &b.ty) {
            Some(Type::Callable { params, ret }) => {
                // Generate function stub with actual signature
                let params_str = params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, self.type_to_stub_annotation(&p.ty)))
                    .collect::<Vec<_>>()
                    .join(", ");

                let ret_str = self.type_to_stub_annotation(ret);

                format!(
                    r#"def {symbol}({params}) -> {ret}: ...
"#,
                    symbol = request.symbol,
                    params = params_str,
                    ret = ret_str
                )
            }
            Some(Type::ClassType { name, .. }) | Some(Type::Instance { name, .. }) => {
                // Generate class stub
                if let Some(class_info) = self.type_context.get_class_info(name) {
                    self.generate_class_stub(&request.symbol, class_info)
                } else {
                    // Fallback
                    format!(
                        r#"class {symbol}:
    """Class {symbol}."""
    ...
"#,
                        symbol = request.symbol
                    )
                }
            }
            _ => {
                // Fallback to generic signature
                format!(
                    r#"from typing import Any

def {symbol}(*args: Any, **kwargs: Any) -> Any: ...
"#,
                    symbol = request.symbol
                )
            }
        };

        let mut stub_file = request.file.clone();
        stub_file.set_extension("pyi");

        CodeGenResult::new(stub, stub_file)
    }

    /// Convert Type to stub annotation string.
    fn type_to_stub_annotation(&self, ty: &Type) -> String {
        use crate::domain::type_system::ty::Type;

        match ty {
            Type::None => "None".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Int => "int".to_string(),
            Type::Float => "float".to_string(),
            Type::Str => "str".to_string(),
            Type::Bytes => "bytes".to_string(),
            Type::List(inner) => format!("list[{}]", self.type_to_stub_annotation(inner)),
            Type::Dict(k, v) => format!(
                "dict[{}, {}]",
                self.type_to_stub_annotation(k),
                self.type_to_stub_annotation(v)
            ),
            Type::Set(inner) => format!("set[{}]", self.type_to_stub_annotation(inner)),
            Type::Tuple(types) => {
                let types_str = types
                    .iter()
                    .map(|t| self.type_to_stub_annotation(t))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("tuple[{}]", types_str)
            }
            Type::Optional(inner) => {
                format!("Optional[{}]", self.type_to_stub_annotation(inner))
            }
            Type::Union(types) => {
                let types_str = types
                    .iter()
                    .map(|t| self.type_to_stub_annotation(t))
                    .collect::<Vec<_>>()
                    .join(" | ");
                types_str
            }
            Type::Callable { params, ret } => {
                let params_str = params
                    .iter()
                    .map(|p| self.type_to_stub_annotation(&p.ty))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "Callable[[{}], {}]",
                    params_str,
                    self.type_to_stub_annotation(ret)
                )
            }
            Type::Instance {
                name, type_args, ..
            } => {
                if type_args.is_empty() {
                    name.clone()
                } else {
                    let args_str = type_args
                        .iter()
                        .map(|t| self.type_to_stub_annotation(t))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}[{}]", name, args_str)
                }
            }
            Type::ClassType { name, .. } => format!("type[{}]", name),
            Type::Any => "Any".to_string(),
            Type::Unknown => "Any".to_string(),
            _ => "Any".to_string(),
        }
    }

    /// Generate stub for a class.
    fn generate_class_stub(&self, name: &str, class_info: &ClassInfo) -> String {
        let mut stub = String::new();

        // Class declaration
        if class_info.bases.is_empty() {
            stub.push_str(&format!("class {}:\n", name));
        } else {
            let bases = class_info.bases.join(", ");
            stub.push_str(&format!("class {}({}):\n", name, bases));
        }

        // Docstring
        stub.push_str(&format!("    \"\"\"Class {}.\"\"\"\n\n", name));

        // Attributes
        for (attr_name, attr_type) in &class_info.attributes {
            stub.push_str(&format!(
                "    {}: {}\n",
                attr_name,
                self.type_to_stub_annotation(attr_type)
            ));
        }

        if !class_info.attributes.is_empty() {
            stub.push_str("\n");
        }

        // Methods
        for (method_name, method_type) in &class_info.methods {
            match method_type {
                Type::Callable { params, ret } => {
                    let params_str = params
                        .iter()
                        .map(|p| format!("{}: {}", p.name, self.type_to_stub_annotation(&p.ty)))
                        .collect::<Vec<_>>()
                        .join(", ");

                    let ret_str = self.type_to_stub_annotation(ret);

                    stub.push_str(&format!(
                        "    def {}({}) -> {}: ...\n",
                        method_name, params_str, ret_str
                    ));
                }
                _ => {
                    stub.push_str(&format!("    def {}(self) -> Any: ...\n", method_name));
                }
            }
        }

        if class_info.methods.is_empty() && class_info.attributes.is_empty() {
            stub.push_str("    ...\n");
        }

        stub
    }

    /// Generate type stub for entire module (.pyi file).
    ///
    /// Note: This is a simplified implementation. For full module stub generation,
    /// we need access to all symbol bindings in the file, which requires passing
    /// FileAnalysis or extending TypeContext with a method to iterate bindings.
    pub(super) fn generate_module_stub(&self, request: &CodeGenRequest) -> CodeGenResult {
        use crate::domain::type_system::ty::Type;
        use std::collections::HashSet;

        let mut stub = String::new();

        // Collect all imports needed
        let mut imports = HashSet::new();
        imports.insert("from typing import Any, Optional, Callable".to_string());

        // Generate stub for the requested symbol if available
        if let Some(binding) = self
            .type_context
            .get_binding(&request.file, &request.symbol)
        {
            let exported_symbols = vec![request.symbol.clone()];

            // Generate stub based on type
            match &binding.ty {
                Type::Callable { params, ret } => {
                    let params_str = params
                        .iter()
                        .map(|p| format!("{}: {}", p.name, self.type_to_stub_annotation(&p.ty)))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let ret_str = self.type_to_stub_annotation(ret);
                    stub.push_str(&format!(
                        "\ndef {}({}) -> {}: ...\n",
                        request.symbol, params_str, ret_str
                    ));
                }
                Type::ClassType {
                    name: class_name, ..
                }
                | Type::Instance {
                    name: class_name, ..
                } => {
                    if let Some(class_info) = self.type_context.get_class_info(class_name) {
                        stub.push_str("\n");
                        stub.push_str(&self.generate_class_stub(&request.symbol, class_info));
                    } else {
                        stub.push_str(&format!("\nclass {}:\n    ...\n", request.symbol));
                    }
                }
                _ => {
                    stub.push_str(&format!("\n{}: Any\n", request.symbol));
                }
            }

            // Add __all__ export list
            let all_list = format!(
                "__all__ = [{}]\n\n",
                exported_symbols
                    .iter()
                    .map(|s| format!("\"{}\"", s))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            stub.insert_str(0, &all_list);
        } else {
            // No binding found - generate minimal stub
            stub.push_str("# Module stub\n");
            stub.push_str(&format!("\n{}: Any\n", request.symbol));
        }

        // Prepend imports
        let imports_str = format!("{}\n\n", imports.into_iter().collect::<Vec<_>>().join("\n"));
        stub.insert_str(0, &imports_str);

        let mut stub_file = request.file.clone();
        stub_file.set_extension("pyi");

        CodeGenResult::new(stub, stub_file)
    }
}
