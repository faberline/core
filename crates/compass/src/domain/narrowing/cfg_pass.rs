use std::collections::HashMap;

use crate::domain::narrowing::block_env::{BlockNarrowEnv, CfgNarrowingResult};
use crate::domain::narrowing::condition::{negate_condition, NarrowingCondition};
use crate::domain::narrowing::narrower::TypeNarrower;
use crate::domain::type_system::ty::Type;
use crate::semantic::pdg::cfg::{BlockId, BlockKind, ControlFlowGraph};

/// CFG-based narrowing pass
///
/// For each IfCondition / LoopCondition block, extracts the condition
/// from the statement text and propagates narrowed types to successor blocks.
pub struct CfgNarrowingPass<'a> {
    cfg: &'a ControlFlowGraph,
    /// Original (pre-narrowing) type environment
    original_types: HashMap<String, Type>,
    /// Source code (for parsing conditions)
    _source: &'a str,
}

impl<'a> CfgNarrowingPass<'a> {
    /// Create a new narrowing pass
    pub fn new(
        cfg: &'a ControlFlowGraph,
        original_types: HashMap<String, Type>,
        source: &'a str,
    ) -> Self {
        Self {
            cfg,
            original_types,
            _source: source,
        }
    }

    /// Run the narrowing pass over the entire CFG
    ///
    /// Uses a worklist algorithm:
    /// 1. Start with empty narrowing environments for all blocks
    /// 2. For each IfCondition/LoopCondition block, compute the condition
    /// 3. Propagate narrowed types to true/false successor blocks
    /// 4. At merge points (join), take the union of narrowed types
    pub fn run(&self) -> CfgNarrowingResult {
        let mut block_envs: HashMap<BlockId, BlockNarrowEnv> = HashMap::new();

        // Initialize: entry block has the original types as its narrowed env
        block_envs.insert(
            self.cfg.entry,
            BlockNarrowEnv {
                narrowed: self.original_types.clone(),
            },
        );

        // Process blocks in RPO-like order (entry first)
        let mut worklist = vec![self.cfg.entry];
        let mut visited = std::collections::HashSet::new();

        while let Some(block_id) = worklist.pop() {
            if !visited.insert(block_id) {
                continue;
            }

            let current_env = block_envs.entry(block_id).or_default().clone();

            let block = match self.cfg.get_block(block_id) {
                Some(b) => b,
                None => continue,
            };

            // Check if this is a conditional block
            let is_conditional = matches!(
                block.kind,
                BlockKind::IfCondition | BlockKind::LoopCondition
            );

            if is_conditional {
                // Extract condition from the block's statement
                if let Some(condition) = self.extract_condition_from_block(block_id) {
                    let negated = negate_condition(&condition);

                    // Apply condition to successors
                    if let Some(edges) = self.cfg.successors.get(&block_id) {
                        for edge in edges {
                            use crate::semantic::pdg::cfg::EdgeKind;

                            let branch_condition = match edge.kind {
                                EdgeKind::TrueBranch => Some(condition.clone()),
                                EdgeKind::FalseBranch => Some(negated.clone()),
                                _ => None,
                            };

                            let successor_env = if let Some(cond) = branch_condition {
                                self.apply_condition_to_env(&current_env, &cond)
                            } else {
                                current_env.clone()
                            };

                            // Merge with existing env if block already has one
                            let merged = if let Some(existing) = block_envs.get(&edge.to) {
                                existing.join(&successor_env)
                            } else {
                                successor_env
                            };

                            block_envs.insert(edge.to, merged);
                            worklist.push(edge.to);
                        }
                    }
                } else {
                    // No condition parsed — propagate current env unchanged
                    self.propagate_unchanged(
                        block_id,
                        &current_env,
                        &mut block_envs,
                        &mut worklist,
                    );
                }
            } else {
                // Non-conditional block: propagate env unchanged
                self.propagate_unchanged(block_id, &current_env, &mut block_envs, &mut worklist);
            }
        }

        CfgNarrowingResult { block_envs }
    }

    /// Extract and parse the narrowing condition from an IfCondition block
    fn extract_condition_from_block(&self, block_id: BlockId) -> Option<NarrowingCondition> {
        let block = self.cfg.get_block(block_id)?;

        // The condition is typically the first (and only) statement in the block
        let stmt = block.statements.first()?;

        // Parse the condition text using the existing parse_condition function
        // We need to parse it as a tree-sitter node. Since we have the source,
        // we use a lightweight text-based parsing approach here.
        let condition = self.parse_condition_text(&stmt.text)?;

        Some(condition)
    }

    /// Parse a condition from its text representation
    ///
    /// This is a lightweight text-based parser for common patterns.
    /// For full AST accuracy, use parse_condition() with the tree-sitter node.
    fn parse_condition_text(&self, text: &str) -> Option<NarrowingCondition> {
        let text = text.trim();

        // isinstance(x, T) or isinstance(x, (T1, T2))
        if text.starts_with("isinstance(") {
            return self.parse_isinstance_text(text);
        }

        // x is None
        if let Some(var) = text.strip_suffix(" is None") {
            let var = var.trim();
            if is_simple_identifier(var) {
                return Some(NarrowingCondition::IsNone {
                    var_name: var.to_string(),
                });
            }
        }

        // x is not None
        if let Some(var) = text.strip_suffix(" is not None") {
            let var = var.trim();
            if is_simple_identifier(var) {
                return Some(NarrowingCondition::IsNotNone {
                    var_name: var.to_string(),
                });
            }
        }

        // x == None or x != None (less common but valid)
        if let Some(var) = text.strip_suffix(" == None") {
            let var = var.trim();
            if is_simple_identifier(var) {
                return Some(NarrowingCondition::IsNone {
                    var_name: var.to_string(),
                });
            }
        }

        if let Some(var) = text.strip_suffix(" != None") {
            let var = var.trim();
            if is_simple_identifier(var) {
                return Some(NarrowingCondition::IsNotNone {
                    var_name: var.to_string(),
                });
            }
        }

        // callable(x)
        if text.starts_with("callable(") && text.ends_with(')') {
            let inner = &text[9..text.len() - 1];
            if is_simple_identifier(inner.trim()) {
                return Some(NarrowingCondition::IsCallable {
                    var_name: inner.trim().to_string(),
                });
            }
        }

        // hasattr(x, "attr")
        if text.starts_with("hasattr(") {
            return self.parse_hasattr_text(text);
        }

        // Simple identifier (truthiness)
        if is_simple_identifier(text) {
            return Some(NarrowingCondition::Truthy {
                var_name: text.to_string(),
            });
        }

        // not x (falsiness)
        if let Some(inner) = text.strip_prefix("not ") {
            let inner = inner.trim();
            if is_simple_identifier(inner) {
                return Some(NarrowingCondition::Falsy {
                    var_name: inner.to_string(),
                });
            }
        }

        None
    }

    fn parse_isinstance_text(&self, text: &str) -> Option<NarrowingCondition> {
        // isinstance(var, Type) or isinstance(var, (T1, T2))
        let inner = text.strip_prefix("isinstance(")?.strip_suffix(')')?;

        let comma_pos = inner.find(',')?;
        let var_part = inner[..comma_pos].trim();
        let type_part = inner[comma_pos + 1..].trim();

        if !is_simple_identifier(var_part) {
            return None;
        }

        let types = if type_part.starts_with('(') && type_part.ends_with(')') {
            // Tuple of types
            let inner_types = &type_part[1..type_part.len() - 1];
            inner_types
                .split(',')
                .map(|t| parse_simple_type_from_name(t.trim()))
                .collect()
        } else {
            vec![parse_simple_type_from_name(type_part)]
        };

        Some(NarrowingCondition::IsInstance {
            var_name: var_part.to_string(),
            types,
        })
    }

    fn parse_hasattr_text(&self, text: &str) -> Option<NarrowingCondition> {
        // hasattr(var, "attr_name")
        let inner = text.strip_prefix("hasattr(")?.strip_suffix(')')?;

        let comma_pos = inner.find(',')?;
        let var_part = inner[..comma_pos].trim();
        let attr_part = inner[comma_pos + 1..].trim();

        if !is_simple_identifier(var_part) {
            return None;
        }

        let attr_name = attr_part
            .trim_start_matches(|c| c == '"' || c == '\'')
            .trim_end_matches(|c| c == '"' || c == '\'')
            .to_string();

        Some(NarrowingCondition::HasAttr {
            var_name: var_part.to_string(),
            attr_name,
        })
    }

    /// Apply a narrowing condition to a BlockNarrowEnv
    fn apply_condition_to_env(
        &self,
        env: &BlockNarrowEnv,
        condition: &NarrowingCondition,
    ) -> BlockNarrowEnv {
        let mut narrower = TypeNarrower::new();
        narrower.push_scope();

        // Merge env types into original_types for the narrower
        let mut combined = self.original_types.clone();
        for (var, ty) in &env.narrowed {
            combined.insert(var.clone(), ty.clone());
        }

        narrower.apply_condition(condition, &combined);

        let scope = narrower.pop_scope().unwrap_or_default();

        // Build the new env: start with the existing env, overlay narrowed types
        let mut new_narrowed = env.narrowed.clone();
        for (var, ty) in scope.narrowed {
            new_narrowed.insert(var, ty);
        }

        BlockNarrowEnv {
            narrowed: new_narrowed,
        }
    }

    /// Propagate the current env unchanged to all successor blocks
    fn propagate_unchanged(
        &self,
        block_id: BlockId,
        current_env: &BlockNarrowEnv,
        block_envs: &mut HashMap<BlockId, BlockNarrowEnv>,
        worklist: &mut Vec<BlockId>,
    ) {
        if let Some(edges) = self.cfg.successors.get(&block_id) {
            for edge in edges {
                let merged = if let Some(existing) = block_envs.get(&edge.to) {
                    existing.join(current_env)
                } else {
                    current_env.clone()
                };

                block_envs.insert(edge.to, merged);
                worklist.push(edge.to);
            }
        }
    }
}

/// Check if a string is a simple Python identifier
fn is_simple_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Parse a simple type name to a Type
fn parse_simple_type_from_name(name: &str) -> Type {
    match name {
        "int" => Type::Int,
        "float" => Type::Float,
        "str" => Type::Str,
        "bool" => Type::Bool,
        "bytes" => Type::Bytes,
        "list" | "List" => Type::List(Box::new(Type::Unknown)),
        "dict" | "Dict" => Type::Dict(Box::new(Type::Unknown), Box::new(Type::Unknown)),
        "set" | "Set" => Type::Set(Box::new(Type::Unknown)),
        "tuple" | "Tuple" => Type::Tuple(vec![]),
        "None" => Type::None,
        _ => Type::Instance {
            name: name.to_string(),
            module: None,
            type_args: vec![],
        },
    }
}

#[cfg(test)]
mod tests;
