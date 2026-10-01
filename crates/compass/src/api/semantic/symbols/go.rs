//! Go symbol extraction visitor methods
//!
//! Extracts symbols from Go AST nodes:
//! - Package declarations (package_clause) → Module
//! - Functions (function_declaration) → Function
//! - Methods (method_declaration) → Function
//! - Types (type_declaration → type_spec) → Class/Struct/Interface
//! - Constants (const_declaration) → Const
//! - Variables (var_declaration) → Variable
//! - Imports (import_declaration) → Import
