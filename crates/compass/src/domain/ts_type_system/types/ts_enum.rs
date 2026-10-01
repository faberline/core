/// TypeScript enum definition
#[derive(Debug, Clone, PartialEq)]
pub struct TsEnum {
    /// Enum name
    pub name: String,
    /// Members (name -> value type)
    pub members: Vec<(String, Option<TsEnumValue>)>,
    /// Is const enum?
    pub is_const: bool,
}

/// TypeScript enum value
#[derive(Debug, Clone, PartialEq)]
pub enum TsEnumValue {
    Number(i64),
    String(String),
}
