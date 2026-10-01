use super::*;

#[test]
fn test_scan_pub_struct() {
    let mut scanner = RustScanner::new().unwrap();
    let source = r#"
/// A user in the system.
#[derive(Clone, Debug)]
pub struct User {
    pub name: String,
    pub age: u32,
    password: String,
}
"#;
    let (exports, _) = scanner.scan_file(source).unwrap();

    assert_eq!(exports.structs.len(), 1);
    let user = &exports.structs[0];
    assert_eq!(user.name, "User");
    assert!(user
        .docstring
        .as_ref()
        .unwrap()
        .contains("user in the system"));
    assert!(user.has_clone);
    assert_eq!(user.fields.len(), 3);

    // Check field visibility
    assert!(user.fields[0].is_public); // name
    assert!(user.fields[1].is_public); // age
    assert!(!user.fields[2].is_public); // password
}

#[test]
fn test_scan_pub_enum() {
    let mut scanner = RustScanner::new().unwrap();
    let source = r#"
/// Loading strategy for relationships.
pub enum LoadingStrategy {
    /// Load lazily on access
    Lazy,
    /// Load eagerly with parent
    Eager,
    /// Use subquery for loading
    Subquery,
}
"#;
    let (exports, _) = scanner.scan_file(source).unwrap();

    assert_eq!(exports.enums.len(), 1);
    let e = &exports.enums[0];
    assert_eq!(e.name, "LoadingStrategy");
    assert!(e.is_simple);
    assert_eq!(e.variants.len(), 3);
    assert_eq!(e.variants[0].name, "Lazy");
}

#[test]
fn test_scan_pub_function() {
    let mut scanner = RustScanner::new().unwrap();
    let source = r#"
/// Connect to the database.
pub async fn connect(url: &str) -> Result<Connection> {
    todo!()
}
"#;
    let (exports, _) = scanner.scan_file(source).unwrap();

    assert_eq!(exports.functions.len(), 1);
    let f = &exports.functions[0];
    assert_eq!(f.name, "connect");
    assert!(f.is_async);
    assert_eq!(f.params.len(), 1);
    assert_eq!(f.params[0].name, "url");
}

#[test]
fn test_scan_impl_methods() {
    let mut scanner = RustScanner::new().unwrap();
    let source = r#"
pub struct QueryBuilder {
    table: String,
}

impl QueryBuilder {
    /// Create a new query builder.
    pub fn new(table: &str) -> Self {
        todo!()
    }

    /// Add SELECT columns.
    pub fn select(&self, columns: Vec<String>) -> Self {
        todo!()
    }

    /// Build the query.
    pub async fn execute(&mut self) -> Result<Vec<Row>> {
        todo!()
    }
}
"#;
    let (mut exports, impl_methods) = scanner.scan_file(source).unwrap();

    // Attach methods to struct
    for s in &mut exports.structs {
        if let Some(methods) = impl_methods.get(&s.name) {
            s.methods = methods.clone();
        }
    }

    assert_eq!(exports.structs.len(), 1);
    let qb = &exports.structs[0];
    assert_eq!(qb.name, "QueryBuilder");
    assert_eq!(qb.methods.len(), 3);

    // Check new() is static
    let new_method = qb.methods.iter().find(|m| m.name == "new").unwrap();
    assert!(new_method.is_static);

    // Check select() takes &self
    let select_method = qb.methods.iter().find(|m| m.name == "select").unwrap();
    assert!(select_method.takes_self);

    // Check execute() is async and takes &mut self
    let execute_method = qb.methods.iter().find(|m| m.name == "execute").unwrap();
    assert!(execute_method.is_async);
    assert!(execute_method.takes_mut_self);
}

#[test]
fn test_generic_struct_detection() {
    let mut scanner = RustScanner::new().unwrap();
    let source = r#"
pub struct Container<T> {
    data: T,
}
"#;
    let (exports, _) = scanner.scan_file(source).unwrap();

    assert_eq!(exports.structs.len(), 1);
    assert_eq!(exports.structs[0].kind, StructKind::Generic);
}

#[test]
fn test_non_simple_enum() {
    let mut scanner = RustScanner::new().unwrap();
    let source = r#"
pub enum Value {
    Int(i64),
    String(String),
    List(Vec<Value>),
}
"#;
    let (exports, _) = scanner.scan_file(source).unwrap();

    assert_eq!(exports.enums.len(), 1);
    assert!(!exports.enums[0].is_simple);
}
