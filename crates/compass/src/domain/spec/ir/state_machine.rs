//! State machine IR (from Mermaid stateDiagram).

// ============================================================================
// State Machine Specification (from Mermaid stateDiagram)
// ============================================================================

/// State machine specification
#[derive(Debug, Clone, Default)]
pub struct StateMachineSpec {
    /// State machine name
    pub name: String,
    /// States
    pub states: Vec<StateDef>,
    /// Transitions
    pub transitions: Vec<TransitionDef>,
    /// Initial state
    pub initial_state: Option<String>,
    /// Final states
    pub final_states: Vec<String>,
}

/// State definition
#[derive(Debug, Clone)]
pub struct StateDef {
    pub name: String,
    pub description: Option<String>,
    /// Entry action
    pub on_enter: Option<String>,
    /// Exit action
    pub on_exit: Option<String>,
    /// Nested state machine
    pub nested: Option<Box<StateMachineSpec>>,
}

/// State transition
#[derive(Debug, Clone)]
pub struct TransitionDef {
    pub from: String,
    pub to: String,
    pub event: Option<String>,
    pub guard: Option<String>,
    pub action: Option<String>,
}

impl StateMachineSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }
}
