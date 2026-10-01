mod type_provider;

use std::collections::HashMap;

use crate::type_inference::Type;

// ============================================================================
// Django Support
// ============================================================================

/// Django type provider.
pub struct DjangoTypeProvider {
    /// Model definitions
    models: HashMap<String, DjangoModel>,
}

/// Django model definition.
#[derive(Debug, Clone)]
pub struct DjangoModel {
    /// Model name
    pub name: String,
    /// Fields
    pub fields: HashMap<String, DjangoField>,
    /// Related models
    pub relations: Vec<DjangoRelation>,
}

/// Django field.
#[derive(Debug, Clone)]
pub struct DjangoField {
    /// Field name
    pub name: String,
    /// Field type
    pub field_type: DjangoFieldType,
    /// Is nullable
    pub null: bool,
    /// Has default
    pub has_default: bool,
}

/// Django field type.
#[derive(Debug, Clone)]
pub enum DjangoFieldType {
    CharField,
    TextField,
    IntegerField,
    FloatField,
    BooleanField,
    DateField,
    DateTimeField,
    ForeignKey(String),
    OneToOneField(String),
    ManyToManyField(String),
    Custom(String),
}

/// Django relation.
#[derive(Debug, Clone)]
pub struct DjangoRelation {
    /// Relation name
    pub name: String,
    /// Related model
    pub related_model: String,
    /// Relation type
    pub relation_type: DjangoRelationType,
}

/// Django relation type.
#[derive(Debug, Clone)]
pub enum DjangoRelationType {
    ForeignKey,
    OneToOne,
    ManyToMany,
}

impl DjangoTypeProvider {
    /// Create a new provider.
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    /// Register a model.
    pub fn register_model(&mut self, model: DjangoModel) {
        self.models.insert(model.name.clone(), model);
    }

    /// Get model info.
    pub fn get_model(&self, name: &str) -> Option<&DjangoModel> {
        self.models.get(name)
    }
}

impl Default for DjangoTypeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DjangoTypeProvider {
    fn field_type_to_type(&self, field_type: &DjangoFieldType) -> Type {
        match field_type {
            DjangoFieldType::CharField | DjangoFieldType::TextField => Type::Str,
            DjangoFieldType::IntegerField => Type::Int,
            DjangoFieldType::FloatField => Type::Float,
            DjangoFieldType::BooleanField => Type::Bool,
            DjangoFieldType::DateField | DjangoFieldType::DateTimeField => Type::Instance {
                name: "datetime".to_string(),
                module: Some("datetime".to_string()),
                type_args: Vec::new(),
            },
            DjangoFieldType::ForeignKey(model) | DjangoFieldType::OneToOneField(model) => {
                Type::Instance {
                    name: model.clone(),
                    module: None,
                    type_args: Vec::new(),
                }
            }
            DjangoFieldType::ManyToManyField(model) => Type::List(Box::new(Type::Instance {
                name: model.clone(),
                module: None,
                type_args: Vec::new(),
            })),
            DjangoFieldType::Custom(name) => Type::Instance {
                name: name.clone(),
                module: None,
                type_args: Vec::new(),
            },
        }
    }

    /// Parse Django models from Python source code.
    /// This is a simplified parser that looks for models.Model subclasses and field definitions.
    pub fn parse_models_from_source(&mut self, source: &str, module_path: &str) {
        let lines: Vec<&str> = source.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Look for class definitions that inherit from models.Model
            if line.starts_with("class ") && line.contains("(") {
                if let Some(model_name) = self.extract_model_name(line) {
                    // Check if it inherits from models.Model
                    if line.contains("models.Model") || line.contains("Model") {
                        // Parse the model body
                        let model = self.parse_model_body(&lines, i + 1, &model_name, module_path);
                        self.register_model(model);
                    }
                }
            }

            i += 1;
        }
    }

    fn extract_model_name(&self, line: &str) -> Option<String> {
        // Extract "User" from "class User(models.Model):"
        if let Some(start) = line.find("class ") {
            let rest = &line[start + 6..];
            if let Some(paren) = rest.find('(') {
                return Some(rest[..paren].trim().to_string());
            }
        }
        None
    }

    fn parse_model_body(
        &self,
        lines: &[&str],
        start_idx: usize,
        model_name: &str,
        _module_path: &str,
    ) -> DjangoModel {
        let mut fields = HashMap::new();
        let mut relations = Vec::new();
        let mut i = start_idx;

        // Determine indentation level of the class body
        let base_indent = lines
            .get(start_idx)
            .and_then(|line| {
                let trimmed = line.trim_start();
                if !trimmed.is_empty() {
                    Some(line.len() - trimmed.len())
                } else {
                    None
                }
            })
            .unwrap_or(4);

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim_start();

            // Stop if we've left the class body (de-dented to class level or less)
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                let current_indent = line.len() - trimmed.len();
                if current_indent < base_indent {
                    break;
                }
            }

            // Look for field definitions (e.g., "name = models.CharField(...)")
            if trimmed.contains(" = models.") {
                if let Some(field) = self.parse_field_definition(trimmed) {
                    if let DjangoFieldType::ForeignKey(ref related_model) = field.field_type {
                        relations.push(DjangoRelation {
                            name: field.name.clone(),
                            related_model: related_model.clone(),
                            relation_type: DjangoRelationType::ForeignKey,
                        });
                    } else if let DjangoFieldType::OneToOneField(ref related_model) =
                        field.field_type
                    {
                        relations.push(DjangoRelation {
                            name: field.name.clone(),
                            related_model: related_model.clone(),
                            relation_type: DjangoRelationType::OneToOne,
                        });
                    } else if let DjangoFieldType::ManyToManyField(ref related_model) =
                        field.field_type
                    {
                        relations.push(DjangoRelation {
                            name: field.name.clone(),
                            related_model: related_model.clone(),
                            relation_type: DjangoRelationType::ManyToMany,
                        });
                    }
                    fields.insert(field.name.clone(), field);
                }
            }

            i += 1;
        }

        DjangoModel {
            name: model_name.to_string(),
            fields,
            relations,
        }
    }

    fn parse_field_definition(&self, line: &str) -> Option<DjangoField> {
        // Extract field name from "name = models.CharField(...)"
        if let Some(eq_pos) = line.find('=') {
            let field_name = line[..eq_pos].trim().to_string();
            let rest = line[eq_pos + 1..].trim();

            // Determine field type
            let field_type = if rest.contains("CharField") {
                DjangoFieldType::CharField
            } else if rest.contains("TextField") {
                DjangoFieldType::TextField
            } else if rest.contains("IntegerField")
                || rest.contains("AutoField")
                || rest.contains("BigAutoField")
            {
                DjangoFieldType::IntegerField
            } else if rest.contains("FloatField") || rest.contains("DecimalField") {
                DjangoFieldType::FloatField
            } else if rest.contains("BooleanField") {
                DjangoFieldType::BooleanField
            } else if rest.contains("DateTimeField") {
                DjangoFieldType::DateTimeField
            } else if rest.contains("DateField") {
                DjangoFieldType::DateField
            } else if rest.contains("ForeignKey") {
                let related_model = self
                    .extract_related_model(rest)
                    .unwrap_or_else(|| "Unknown".to_string());
                DjangoFieldType::ForeignKey(related_model)
            } else if rest.contains("OneToOneField") {
                let related_model = self
                    .extract_related_model(rest)
                    .unwrap_or_else(|| "Unknown".to_string());
                DjangoFieldType::OneToOneField(related_model)
            } else if rest.contains("ManyToManyField") {
                let related_model = self
                    .extract_related_model(rest)
                    .unwrap_or_else(|| "Unknown".to_string());
                DjangoFieldType::ManyToManyField(related_model)
            } else {
                DjangoFieldType::Custom("Unknown".to_string())
            };

            // Check for null=True
            let null = rest.contains("null=True") || rest.contains("null = True");

            // Check for default value
            let has_default = rest.contains("default=") || rest.contains("default =");

            return Some(DjangoField {
                name: field_name,
                field_type,
                null,
                has_default,
            });
        }

        None
    }

    fn extract_related_model(&self, field_definition: &str) -> Option<String> {
        // Extract "User" from "ForeignKey(User, ...)" or "ForeignKey('User', ...)"
        if let Some(start) = field_definition.find('(') {
            let rest = &field_definition[start + 1..];
            if let Some(end) = rest.find(',').or_else(|| rest.find(')')) {
                let model_ref = rest[..end].trim();
                // Remove quotes if present
                let model_name = model_ref.trim_matches(|c| c == '\'' || c == '"');
                // Handle "self" reference
                if model_name == "self" || model_name == "'self'" {
                    return Some("self".to_string());
                }
                return Some(model_name.to_string());
            }
        }
        None
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests;
