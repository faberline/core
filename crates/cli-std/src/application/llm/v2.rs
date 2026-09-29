use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Result};

use super::Format;
use crate::domain::llm::v2::{ProviderContent, Topic, PROTOCOL};

#[derive(Debug, Clone)]
pub struct ProtocolDocument {
    project: String,
    topics: Vec<Topic>,
    providers: BTreeMap<String, Vec<ProviderContent>>,
}

impl ProtocolDocument {
    pub fn new(project: impl Into<String>, topics: Vec<Topic>) -> Result<Self> {
        let document = Self {
            project: project.into(),
            topics,
            providers: BTreeMap::new(),
        };
        document.validate()?;
        Ok(document)
    }

    /// Compose a legacy shared-library topic into one typed app topic.
    ///
    /// Provider order follows call order. The provider remains library-owned;
    /// this document only selects the task topic that includes it.
    pub fn with_topic_provider(mut self, topic: &str, provider: &super::Topic) -> Result<Self> {
        self.topic(topic)?;
        if provider.id.trim().is_empty() {
            bail!("LLM provider id cannot be empty");
        }
        if provider.body.trim().is_empty() {
            bail!("LLM provider `{}` markdown cannot be empty", provider.id);
        }

        let providers = self.providers.entry(topic.to_string()).or_default();
        if providers.iter().any(|entry| entry.id == provider.id) {
            bail!(
                "duplicate LLM provider `{}` for topic `{topic}`",
                provider.id
            );
        }
        providers.push(ProviderContent {
            id: provider.id.to_string(),
            summary: provider.summary.to_string(),
            markdown: provider.body.to_string(),
        });
        Ok(self)
    }

    pub fn topics(&self) -> &[Topic] {
        &self.topics
    }

    pub fn render(&self, topic: &str, format: Format) -> Result<String> {
        let markdown = if topic == "outline" {
            self.outline_markdown()
        } else {
            self.topic_markdown(self.topic(topic)?)
        };
        match format {
            Format::Md => Ok(markdown),
            Format::Json => {
                if topic == "outline" {
                    serde_json::to_string_pretty(&serde_json::json!({
                        "topic": "outline",
                        "markdown": markdown,
                        "protocol": PROTOCOL,
                        "tasks": self.topics.iter().map(|entry| &entry.task).collect::<Vec<_>>(),
                    }))
                    .map_err(Into::into)
                } else {
                    let entry = self.topic(topic)?;
                    let mut envelope = serde_json::json!({
                        "topic": topic,
                        "markdown": markdown,
                        "protocol": PROTOCOL,
                        "task": &entry.task,
                        "runbook": &entry.runbook,
                    });
                    if let Some(providers) = self.providers.get(topic) {
                        envelope
                            .as_object_mut()
                            .expect("LLM detail envelope is an object")
                            .insert("providers".into(), serde_json::json!(providers));
                    }
                    serde_json::to_string_pretty(&envelope).map_err(Into::into)
                }
            }
        }
    }

    fn topic(&self, id: &str) -> Result<&Topic> {
        self.topics
            .iter()
            .find(|entry| entry.task.topic == id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "unknown llm topic `{id}`; run `{} llm --topic outline`",
                    self.project
                )
            })
    }

    fn validate(&self) -> Result<()> {
        if self.project.trim().is_empty() {
            bail!("LLM protocol project cannot be empty");
        }
        let mut ids = BTreeSet::new();
        let mut topics = BTreeSet::new();
        for entry in &self.topics {
            if entry.task.id.trim().is_empty() || entry.task.topic.trim().is_empty() {
                bail!("LLM task id and topic cannot be empty");
            }
            if !ids.insert(entry.task.id.as_str()) {
                bail!("duplicate LLM task id `{}`", entry.task.id);
            }
            if !topics.insert(entry.task.topic.as_str()) {
                bail!("duplicate LLM topic `{}`", entry.task.topic);
            }
            for step in &entry.runbook.steps {
                if step.command.is_some() && step.command_template.is_some() {
                    bail!(
                        "step `{}` cannot contain both command and command_template",
                        step.id
                    );
                }
                if let Some(command) = &step.command {
                    if !step.inputs.is_empty() || command.contains('<') || command.contains('{') {
                        bail!("step `{}` command must be fully bound; use command_template with typed inputs", step.id);
                    }
                }
                if step.command_template.is_some() && step.inputs.is_empty() {
                    bail!("step `{}` command_template requires typed inputs", step.id);
                }
                if let Some(template) = &step.command_template {
                    let declared = step
                        .inputs
                        .iter()
                        .map(|input| input.name.as_str())
                        .collect::<BTreeSet<_>>();
                    let referenced = template_placeholders(template)?;
                    if declared != referenced {
                        bail!(
                            "step `{}` command_template placeholders must exactly match its typed inputs",
                            step.id
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn outline_markdown(&self) -> String {
        let mut out = format!("# {} task navigation\n\n", self.project);
        out.push_str("Select the smallest task, then read its typed runbook:\n\n");
        for entry in &self.topics {
            out.push_str(&format!(
                "- `{}` — {} (`{} llm --topic {}`)\n",
                entry.task.id, entry.task.use_when, self.project, entry.task.topic
            ));
        }
        out.push_str(&format!(
            "\nUse `{} llm --topic <topic> --format json` for {} data.\n",
            self.project, PROTOCOL
        ));
        out
    }

    fn topic_markdown(&self, entry: &Topic) -> String {
        let runbook = &entry.runbook;
        let mut out = format!(
            "# {} — {}\n\n{}\n",
            self.project, entry.task.id, runbook.purpose
        );
        markdown_list(&mut out, "Preconditions", &runbook.preconditions);
        if !runbook.inputs.is_empty() {
            out.push_str("\n## Inputs\n\n");
            for input in &runbook.inputs {
                out.push_str(&format!(
                    "- `{}` ({}, {}) — {}\n",
                    input.name,
                    input.value_type,
                    if input.required {
                        "required"
                    } else {
                        "optional"
                    },
                    input.description
                ));
            }
        }
        markdown_list(&mut out, "Constraints", &runbook.constraints);
        if !runbook.steps.is_empty() {
            out.push_str("\n## Steps\n\n");
            for step in &runbook.steps {
                out.push_str(&format!("1. {}\n", step.instruction));
                if let Some(command) = &step.command {
                    out.push_str(&format!("   `{command}`\n"));
                }
                if let Some(template) = &step.command_template {
                    out.push_str(&format!("   Template: `{template}`\n"));
                }
            }
        }
        markdown_list(&mut out, "Verification", &runbook.verification);
        markdown_list(&mut out, "References", &runbook.references);
        if let Some(providers) = self.providers.get(&entry.task.topic) {
            out.push_str("\n## Shared providers\n");
            for provider in providers {
                out.push_str(&format!(
                    "\n### `{}`\n\n{}\n\n{}\n",
                    provider.id,
                    provider.summary,
                    provider.markdown.trim()
                ));
            }
        }
        out
    }
}

fn template_placeholders(template: &str) -> Result<BTreeSet<&str>> {
    let mut placeholders = BTreeSet::new();
    let mut remaining = template;
    while let Some(open) = remaining.find('{') {
        let after_open = &remaining[open + 1..];
        let Some(close) = after_open.find('}') else {
            bail!("command_template has an unclosed placeholder");
        };
        let placeholder = &after_open[..close];
        if placeholder.trim().is_empty()
            || !placeholder
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            bail!("command_template has an invalid placeholder `{{{placeholder}}}`");
        }
        placeholders.insert(placeholder);
        remaining = &after_open[close + 1..];
    }
    if remaining.contains('}') {
        bail!("command_template has a closing brace without an opening brace");
    }
    Ok(placeholders)
}

fn markdown_list(out: &mut String, heading: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    out.push_str(&format!("\n## {heading}\n\n"));
    for value in values {
        out.push_str(&format!("- {value}\n"));
    }
}

#[cfg(test)]
mod tests;
