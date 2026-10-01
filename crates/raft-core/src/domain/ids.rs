/// Stable node identity (in k8s, the StatefulSet ordinal).
pub type NodeId = u64;
pub type Term = u64;
/// 1-based Raft log index; 0 means "before the first entry".
pub type Index = u64;
