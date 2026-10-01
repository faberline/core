//! classDiagram parsing.

use super::{MermaidError, MermaidParser};
use crate::domain::spec::ir::{
    DataModelSpec, FieldConstraints, FieldDef, MethodDef, ModelDef, ParamDef, RelationType,
    Relationship, Visibility,
};
use crate::type_inference::Type;

impl MermaidParser {
    /// Parse classDiagram
    pub(super) fn parse_class_diagram(&self, content: &str) -> Result<DataModelSpec, MermaidError> {
        let mut spec = DataModelSpec::new();
        let mut current_class: Option<ModelDef> = None;

        for line in content.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() || line.starts_with("%%") {
                continue;
            }

            // Class definition: class ClassName {
            if line.starts_with("class ") {
                // Save previous class
                if let Some(model) = current_class.take() {
                    spec.add_model(model);
                }

                let class_name = self.extract_class_name(line)?;
                current_class = Some(ModelDef {
                    name: class_name,
                    ..Default::default()
                });
            }
            // End of class block
            else if line == "}" {
                if let Some(model) = current_class.take() {
                    spec.add_model(model);
                }
            }
            // Member inside class
            else if let Some(ref mut model) = current_class {
                if let Some(member) = self.parse_class_member(line)? {
                    match member {
                        ClassMember::Field(field) => model.fields.push(field),
                        ClassMember::Method(method) => model.methods.push(method),
                    }
                }
            }
            // Relationship: ClassA --|> ClassB
            else if self.is_relationship(line) {
                if let Some(rel) = self.parse_relationship(line)? {
                    spec.relationships.push(rel);
                }
            }
        }

        // Save last class if not closed
        if let Some(model) = current_class {
            spec.add_model(model);
        }

        Ok(spec)
    }

    /// Extract class name from "class ClassName" or "class ClassName {"
    fn extract_class_name(&self, line: &str) -> Result<String, MermaidError> {
        let without_class = line.strip_prefix("class ").unwrap_or(line);
        let name = without_class
            .split(|c: char| c == '{' || c == ':' || c.is_whitespace())
            .next()
            .ok_or_else(|| MermaidError::SyntaxError("Invalid class definition".into()))?;
        Ok(name.trim().to_string())
    }

    /// Parse class member (field or method)
    fn parse_class_member(&self, line: &str) -> Result<Option<ClassMember>, MermaidError> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(None);
        }

        // Determine visibility
        let (visibility, rest) = if line.starts_with('+') {
            (Visibility::Public, &line[1..])
        } else if line.starts_with('-') {
            (Visibility::Private, &line[1..])
        } else if line.starts_with('#') {
            (Visibility::Protected, &line[1..])
        } else if line.starts_with('~') {
            // Package/internal visibility - map to Protected
            (Visibility::Protected, &line[1..])
        } else {
            (Visibility::Public, line)
        };

        let rest = rest.trim();

        // Check if it's a method (contains parentheses)
        if rest.contains('(') {
            let method = self.parse_method(rest, visibility)?;
            Ok(Some(ClassMember::Method(method)))
        } else {
            // It's a field: type name or name: type
            let field = self.parse_field(rest, visibility)?;
            Ok(Some(ClassMember::Field(field)))
        }
    }

    /// Parse method definition
    fn parse_method(&self, line: &str, visibility: Visibility) -> Result<MethodDef, MermaidError> {
        // Format: methodName(params) returnType or methodName(params): returnType
        let paren_start = line.find('(').unwrap();
        let paren_end = line.find(')').unwrap_or(line.len());

        let name = line[..paren_start].trim().to_string();
        let params_str = &line[paren_start + 1..paren_end];

        // Parse parameters
        let params: Vec<ParamDef> = params_str
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|p| {
                let p = p.trim();
                // Format: name: Type or Type name
                if let Some(colon) = p.find(':') {
                    ParamDef {
                        name: p[..colon].trim().to_string(),
                        ty: self.parse_type(&p[colon + 1..]),
                        default: None,
                    }
                } else {
                    let parts: Vec<&str> = p.split_whitespace().collect();
                    if parts.len() >= 2 {
                        ParamDef {
                            name: parts[1].to_string(),
                            ty: self.parse_type(parts[0]),
                            default: None,
                        }
                    } else {
                        ParamDef {
                            name: parts.get(0).unwrap_or(&"arg").to_string(),
                            ty: Type::Any,
                            default: None,
                        }
                    }
                }
            })
            .collect();

        // Parse return type
        let return_type = if let Some(colon) = line[paren_end..].find(':') {
            self.parse_type(&line[paren_end + colon + 1..])
        } else if line.len() > paren_end + 1 {
            self.parse_type(&line[paren_end + 1..])
        } else {
            Type::None
        };

        Ok(MethodDef {
            name,
            params,
            return_type,
            visibility,
            is_static: false,
            is_async: false,
            description: None,
        })
    }

    /// Parse field definition
    fn parse_field(&self, line: &str, visibility: Visibility) -> Result<FieldDef, MermaidError> {
        // Format: Type name or name: Type
        let (name, ty) = if let Some(colon) = line.find(':') {
            let n = line[..colon].trim();
            let t = line[colon + 1..].trim();
            (n.to_string(), self.parse_type(t))
        } else {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                (parts[1].to_string(), self.parse_type(parts[0]))
            } else {
                (parts.get(0).unwrap_or(&"field").to_string(), Type::Any)
            }
        };

        let required = matches!(visibility, Visibility::Public);

        Ok(FieldDef {
            name,
            ty,
            required,
            default: None,
            description: None,
            constraints: FieldConstraints::default(),
            column_name: None,
            primary_key: false,
            unique: false,
            indexed: false,
            foreign_key: None,
            alias: None,
        })
    }

    /// Parse type string to Type
    pub(super) fn parse_type(&self, s: &str) -> Type {
        let s = s.trim();
        match s.to_lowercase().as_str() {
            "string" | "str" => Type::Str,
            "int" | "integer" | "i32" | "i64" => Type::Int,
            "float" | "double" | "f32" | "f64" | "number" => Type::Float,
            "bool" | "boolean" => Type::Bool,
            "void" | "none" | "()" => Type::None,
            "any" => Type::Any,
            _ => {
                // Check for generic types like List<T>
                if let Some(bracket_start) = s.find('<') {
                    let base = &s[..bracket_start];
                    let inner = &s[bracket_start + 1..s.len() - 1];
                    match base.to_lowercase().as_str() {
                        "list" | "array" | "vec" => Type::List(Box::new(self.parse_type(inner))),
                        "optional" | "option" => Type::Optional(Box::new(self.parse_type(inner))),
                        _ => Type::Instance {
                            name: s.to_string(),
                            module: None,
                            type_args: vec![],
                        },
                    }
                } else if s.ends_with("[]") {
                    let inner = &s[..s.len() - 2];
                    Type::List(Box::new(self.parse_type(inner)))
                } else if s.ends_with('?') {
                    let inner = &s[..s.len() - 1];
                    Type::Optional(Box::new(self.parse_type(inner)))
                } else {
                    Type::Instance {
                        name: s.to_string(),
                        module: None,
                        type_args: vec![],
                    }
                }
            }
        }
    }

    /// Check if line is a relationship
    fn is_relationship(&self, line: &str) -> bool {
        line.contains("--|>")
            || line.contains("<|--")
            || line.contains("--*")
            || line.contains("*--")
            || line.contains("--o")
            || line.contains("o--")
            || line.contains("-->")
            || line.contains("<--")
            || line.contains("--") && !line.starts_with("class ")
    }

    /// Parse relationship line
    fn parse_relationship(&self, line: &str) -> Result<Option<Relationship>, MermaidError> {
        // Find relationship marker
        let (from, to, rel_type) = if line.contains("--|>") {
            let parts: Vec<&str> = line.split("--|>").collect();
            (parts[0].trim(), parts[1].trim(), RelationType::OneToOne) // Inheritance
        } else if line.contains("<|--") {
            let parts: Vec<&str> = line.split("<|--").collect();
            (parts[1].trim(), parts[0].trim(), RelationType::OneToOne) // Inheritance (reverse)
        } else if line.contains("--*") {
            let parts: Vec<&str> = line.split("--*").collect();
            (parts[0].trim(), parts[1].trim(), RelationType::OneToMany) // Composition
        } else if line.contains("*--") {
            let parts: Vec<&str> = line.split("*--").collect();
            (parts[1].trim(), parts[0].trim(), RelationType::ManyToOne)
        } else if line.contains("--o") {
            let parts: Vec<&str> = line.split("--o").collect();
            (parts[0].trim(), parts[1].trim(), RelationType::OneToMany) // Aggregation
        } else if line.contains("o--") {
            let parts: Vec<&str> = line.split("o--").collect();
            (parts[1].trim(), parts[0].trim(), RelationType::ManyToOne)
        } else if line.contains("--") {
            let parts: Vec<&str> = line.split("--").collect();
            if parts.len() >= 2 {
                (parts[0].trim(), parts[1].trim(), RelationType::OneToOne)
            } else {
                return Ok(None);
            }
        } else {
            return Ok(None);
        };

        // Clean up names (remove labels like : "label")
        let from_clean = from.split(':').next().unwrap_or(from).trim();
        let to_clean = to.split(':').next().unwrap_or(to).trim();

        Ok(Some(Relationship {
            from_model: from_clean.to_string(),
            from_field: "id".to_string(),
            to_model: to_clean.to_string(),
            to_field: "id".to_string(),
            rel_type,
        }))
    }
}

/// Enum for class members
enum ClassMember {
    Field(FieldDef),
    Method(MethodDef),
}
