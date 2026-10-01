//! Terraform/HCL symbol extraction via tree-sitter-hcl
//!
//! Extracts symbols from HCL ASTs:
//! - Resources: `resource "type" "name" { ... }`
//! - Data sources: `data "type" "name" { ... }`
//! - Variables: `variable "name" { ... }`
//! - Outputs: `output "name" { ... }`
//! - Locals: `locals { name = ... }`
//! - Modules: `module "name" { ... }`
