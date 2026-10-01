/// A command supplied to the deterministic state-machine host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateMachineOperation {
    Command(Vec<u8>),
}

impl From<Vec<u8>> for StateMachineOperation {
    fn from(command: Vec<u8>) -> Self {
        Self::Command(command)
    }
}
