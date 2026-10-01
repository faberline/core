//! erDiagram parsing.

use super::{MermaidError, MermaidParser};
use crate::domain::spec::ir::{
    DataModelSpec, FieldConstraints, FieldDef, ModelDef, RelationType, Relationship,
};

impl MermaidParser {
    /// Parse erDiagram (Entity-Relationship)
    pub(super) fn parse_er_diagram(&self, content: &str) -> Result<DataModelSpec, MermaidError> {
        let mut spec = DataModelSpec::new();
        let mut current_entity: Option<ModelDef> = None;

        for line in content.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() || line.starts_with("%%") {
                continue;
            }

            // Relationship: Entity1 ||--o{ Entity2 : "relationship"
            if self.is_er_relationship(line) {
                // Save current entity first
                if let Some(entity) = current_entity.take() {
                    spec.add_model(entity);
                }

                if let Some(rel) = self.parse_er_relationship(line)? {
                    spec.relationships.push(rel);
                }
            }
            // Entity definition: EntityName {
            else if line.ends_with('{') {
                if let Some(entity) = current_entity.take() {
                    spec.add_model(entity);
                }
                let name = line.trim_end_matches('{').trim().to_string();
                current_entity = Some(ModelDef {
                    name,
                    table_name: None,
                    ..Default::default()
                });
            }
            // End of entity
            else if line == "}" {
                if let Some(entity) = current_entity.take() {
                    spec.add_model(entity);
                }
            }
            // Field inside entity: type name PK/FK "comment"
            else if let Some(ref mut entity) = current_entity {
                if let Some(field) = self.parse_er_field(line)? {
                    entity.fields.push(field);
                }
            }
        }

        if let Some(entity) = current_entity {
            spec.add_model(entity);
        }

        Ok(spec)
    }

    /// Check if line is an ER relationship
    fn is_er_relationship(&self, line: &str) -> bool {
        line.contains("||")
            || line.contains("}|")
            || line.contains("|{")
            || line.contains("}o")
            || line.contains("o{")
            || line.contains("--")
    }

    /// Parse ER relationship
    fn parse_er_relationship(&self, line: &str) -> Result<Option<Relationship>, MermaidError> {
        // Format: Entity1 ||--o{ Entity2 : "label"
        // ||--|| one to one
        // ||--o{ one to many
        // }o--o{ many to many

        // Find the relationship marker
        let rel_markers = [
            "||--||", "||--o{", "}o--||", "}o--o{", "||--|{", "}|--||", "--",
        ];

        for marker in rel_markers {
            if let Some(pos) = line.find(marker) {
                let left = line[..pos].trim();
                let right_and_label = line[pos + marker.len()..].trim();

                let right = right_and_label
                    .split(':')
                    .next()
                    .unwrap_or(right_and_label)
                    .trim();

                let rel_type = match marker {
                    "||--||" => RelationType::OneToOne,
                    "||--o{" | "||--|{" => RelationType::OneToMany,
                    "}o--||" | "}|--||" => RelationType::ManyToOne,
                    "}o--o{" => RelationType::ManyToMany,
                    _ => RelationType::OneToOne,
                };

                return Ok(Some(Relationship {
                    from_model: left.to_string(),
                    from_field: "id".to_string(),
                    to_model: right.to_string(),
                    to_field: format!("{}_id", left.to_lowercase()),
                    rel_type,
                }));
            }
        }

        Ok(None)
    }

    /// Parse ER field
    fn parse_er_field(&self, line: &str) -> Result<Option<FieldDef>, MermaidError> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            return Ok(None);
        }

        let ty = self.parse_type(parts[0]);
        let name = parts[1].to_string();

        let primary_key = parts.iter().any(|&p| p == "PK");
        let is_foreign_key = parts.iter().any(|&p| p == "FK");
        let unique = parts.iter().any(|&p| p == "UK");

        Ok(Some(FieldDef {
            name,
            ty,
            required: primary_key || !line.contains("NULL"),
            default: None,
            description: None,
            constraints: FieldConstraints::default(),
            column_name: None,
            primary_key,
            unique,
            indexed: primary_key || is_foreign_key,
            foreign_key: None,
            alias: None,
        }))
    }
}
