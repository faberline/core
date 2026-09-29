//! Data model IR: models, fields, enums and relationships.

use crate::type_inference::Type;

/// Data model specification containing models, enums, and relationships
#[derive(Debug, Clone, Default)]
pub struct DataModelSpec {
    /// Model definitions
    pub models: Vec<ModelDef>,
    /// Enum definitions
    pub enums: Vec<EnumDef>,
    /// Relationships between models (for ORM generation)
    pub relationships: Vec<Relationship>,
}

/// Model definition (class/struct/interface)
#[derive(Debug, Clone)]
pub struct ModelDef {
    /// Model name (PascalCase)
    pub name: String,
    /// Optional description/documentation
    pub description: Option<String>,
    /// Field definitions
    pub fields: Vec<FieldDef>,
    /// Method definitions (from Mermaid classDiagram)
    pub methods: Vec<MethodDef>,
    /// Base classes/interfaces this model extends
    pub extends: Vec<String>,
    /// Generic type parameters
    pub type_params: Vec<TypeParam>,
    /// Whether this is an abstract class/protocol
    pub is_abstract: bool,
    /// Database table name (for ORM generation)
    pub table_name: Option<String>,
    /// Collection name (for MongoDB)
    pub collection_name: Option<String>,
}

impl Default for ModelDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            fields: Vec::new(),
            methods: Vec::new(),
            extends: Vec::new(),
            type_params: Vec::new(),
            is_abstract: false,
            table_name: None,
            collection_name: None,
        }
    }
}

/// Field definition with type, constraints, and metadata
#[derive(Debug, Clone)]
pub struct FieldDef {
    /// Field name (snake_case)
    pub name: String,
    /// Field type (uses existing Type IR)
    pub ty: Type,
    /// Whether the field is required
    pub required: bool,
    /// Default value expression (as string)
    pub default: Option<String>,
    /// Field description
    pub description: Option<String>,
    /// Field constraints
    pub constraints: FieldConstraints,
    /// Database column name (if different from field name)
    pub column_name: Option<String>,
    /// Whether this is a primary key
    pub primary_key: bool,
    /// Whether this field has a unique constraint
    pub unique: bool,
    /// Whether this field is indexed
    pub indexed: bool,
    /// Foreign key reference
    pub foreign_key: Option<ForeignKey>,
    /// Alias for serialization
    pub alias: Option<String>,
}

impl Default for FieldDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            ty: Type::Any,
            required: true,
            default: None,
            description: None,
            constraints: FieldConstraints::default(),
            column_name: None,
            primary_key: false,
            unique: false,
            indexed: false,
            foreign_key: None,
            alias: None,
        }
    }
}

/// Field constraints for validation
#[derive(Debug, Clone, Default)]
pub struct FieldConstraints {
    // String constraints
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
    pub format: Option<StringFormat>,

    // Numeric constraints
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub exclusive_minimum: Option<f64>,
    pub exclusive_maximum: Option<f64>,
    pub multiple_of: Option<f64>,

    // Array constraints
    pub min_items: Option<usize>,
    pub max_items: Option<usize>,
    pub unique_items: bool,
}

/// String format types
#[derive(Debug, Clone, PartialEq)]
pub enum StringFormat {
    Email,
    Uri,
    Url,
    Uuid,
    DateTime,
    Date,
    Time,
    Duration,
    Hostname,
    Ipv4,
    Ipv6,
    Regex,
    JsonPointer,
    Custom(String),
}

/// Foreign key reference
#[derive(Debug, Clone)]
pub struct ForeignKey {
    /// Referenced model name
    pub model: String,
    /// Referenced field name
    pub field: String,
    /// On delete action
    pub on_delete: ForeignKeyAction,
    /// On update action
    pub on_update: ForeignKeyAction,
}

/// Foreign key actions
#[derive(Debug, Clone, Default)]
pub enum ForeignKeyAction {
    #[default]
    NoAction,
    Cascade,
    SetNull,
    SetDefault,
    Restrict,
}

/// Method definition (from Mermaid classDiagram)
#[derive(Debug, Clone)]
pub struct MethodDef {
    /// Method name
    pub name: String,
    /// Method parameters
    pub params: Vec<ParamDef>,
    /// Return type
    pub return_type: Type,
    /// Visibility
    pub visibility: Visibility,
    /// Whether this is a static method
    pub is_static: bool,
    /// Whether this is an async method
    pub is_async: bool,
    /// Method description
    pub description: Option<String>,
}

/// Parameter definition
#[derive(Debug, Clone)]
pub struct ParamDef {
    pub name: String,
    pub ty: Type,
    pub default: Option<String>,
}

/// Visibility modifier
#[derive(Debug, Clone, Default)]
pub enum Visibility {
    #[default]
    Public,
    Private,
    Protected,
}

/// Generic type parameter
#[derive(Debug, Clone)]
pub struct TypeParam {
    pub name: String,
    pub bound: Option<Type>,
    pub default: Option<Type>,
}

/// Enum definition
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub description: Option<String>,
    pub variants: Vec<EnumVariant>,
}

/// Enum variant
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<EnumValue>,
    pub description: Option<String>,
}

/// Enum value type
#[derive(Debug, Clone)]
pub enum EnumValue {
    Int(i64),
    String(String),
}

/// Relationship between models (for ERD/ORM)
#[derive(Debug, Clone)]
pub struct Relationship {
    /// Source model name
    pub from_model: String,
    /// Source field name
    pub from_field: String,
    /// Target model name
    pub to_model: String,
    /// Target field name (usually 'id')
    pub to_field: String,
    /// Relationship type
    pub rel_type: RelationType,
}

/// Relationship cardinality
#[derive(Debug, Clone)]
pub enum RelationType {
    /// 1:1 relationship
    OneToOne,
    /// 1:N relationship
    OneToMany,
    /// N:1 relationship
    ManyToOne,
    /// N:M relationship
    ManyToMany,
}

// ============================================================================
// Utility implementations
// ============================================================================

impl DataModelSpec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_model(&mut self, model: ModelDef) {
        self.models.push(model);
    }

    pub fn add_enum(&mut self, enum_def: EnumDef) {
        self.enums.push(enum_def);
    }

    pub fn add_relationship(&mut self, rel: Relationship) {
        self.relationships.push(rel);
    }

    /// Get a model by name
    pub fn get_model(&self, name: &str) -> Option<&ModelDef> {
        self.models.iter().find(|m| m.name == name)
    }

    /// Get an enum by name
    pub fn get_enum(&self, name: &str) -> Option<&EnumDef> {
        self.enums.iter().find(|e| e.name == name)
    }
}

impl ModelDef {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn add_field(&mut self, field: FieldDef) {
        self.fields.push(field);
    }

    pub fn add_method(&mut self, method: MethodDef) {
        self.methods.push(method);
    }
}

impl FieldDef {
    pub fn new(name: impl Into<String>, ty: Type) -> Self {
        Self {
            name: name.into(),
            ty,
            ..Default::default()
        }
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn with_default(mut self, default: impl Into<String>) -> Self {
        self.default = Some(default.into());
        self.required = false;
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn primary_key(mut self) -> Self {
        self.primary_key = true;
        self
    }
}
