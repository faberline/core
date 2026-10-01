//! Type system for Argus
//!
//! This module provides type inference and checking.

pub(crate) mod annotation;
mod builtins;
mod cache;
mod cfg_narrow;
mod check;
mod class_info;
mod config;
mod deep_inference;
mod env;
mod frameworks;
pub(crate) mod imports;
mod incremental;
mod infer;
mod model;
mod modules;
mod mutable_ast;
mod narrow;
mod package_managers;
mod project;
mod propagation;
mod refactoring;
mod semantic_search;
mod stubs;
mod ty;
mod type_env;
mod typeshed;

pub use cache::{AnalysisCache, CacheEntry, ContentHash};
pub use cfg_narrow::{
    apply_typevar_bindings,
    // Protocol structural typing (R2.3)
    check_protocol_satisfaction,
    // Overload resolution (R2.4)
    resolve_overload,
    // TypeVar resolution (R2.2)
    resolve_typevar_bindings,
    // CFG-based narrowing (R2.1)
    BlockNarrowEnv,
    CfgNarrowingPass,
    CfgNarrowingResult,
    ProtocolCheckResult,
    ProtocolMemberError,
};
pub use check::{build_semantic_model, SemanticModelBuilder, TypeChecker, TypeError};
pub use class_info::{ClassInfo, GenericParam};
pub use config::{ArgusConfig, EffectiveConfig, OverrideConfig, PythonEnvConfig};
pub use deep_inference::{
    infer_type_deep, trace_type_chain, CrossFileRef, DeepInferenceResult, DeepTypeInferencer,
    FileAnalysis, GenericKey, ImportGraph, ImportInfo, MethodSignature, ProtocolDef, TypeBinding,
    TypeContext, TypeTraceStep, TypeVarInfo as DeepTypeVarInfo,
};
pub use env::{
    detect_all_venvs, detect_python_environment, detect_with_config, find_site_packages,
    get_venv_python_version, is_venv_directory, DetectedEnv, EnvInfo, VenvType,
};
pub use frameworks::{
    DjangoField, DjangoFieldType, DjangoModel, DjangoRelation, DjangoRelationType,
    DjangoTypeProvider, FastAPIEndpoint, FastAPITypeProvider, Framework, FrameworkDetection,
    FrameworkDetector, FrameworkRegistry, FrameworkTypeProvider, MethodType, PydanticConfig,
    PydanticExtra, PydanticField, PydanticModel, PydanticTypeProvider,
};
pub use imports::{
    Import, ImportResolver, ImportedName, ModuleIndexEntry, ModuleInfo, ModuleLoadState,
};
pub use incremental::{
    AnalysisResult, CachedAnalysis, ChangeKind, ChangeTracker, DependencyGraph, FileChange,
    IncrementalAnalyzer, IncrementalConfig,
};
pub use infer::{TypeInferencer, TypeVarInfo};
pub use model::{
    LiteralInfo, ParamInfo, ScopeId, ScopeInfo, SemanticModel, SemanticSymbolKind, SymbolData,
    SymbolId, SymbolReference, TypeInfo, TypedRange,
};
pub use modules::{ModuleGraph, ModuleNode};
pub use mutable_ast::{
    AstEdit, MutableAst, MutableNode, NodeId, NodeMetadata, NodeRef, Span, TreeDiff,
};
pub use narrow::{NarrowingCondition, TypeNarrower};
pub use package_managers::{
    Dependency, PackageManager, PackageManagerDetection, PackageManagerDetector,
};
pub use project::{ProjectAnalyzer, ProjectConfig};
pub use refactoring::{
    DiagnosticLevel, ImportChange, RefactorDiagnostic, RefactorKind, RefactorOptions,
    RefactorRequest, RefactorResult, RefactoringEngine, SignatureChanges, TextEdit,
};
pub use semantic_search::{
    CallDirection, MatchContext, MatchKind, SearchKind, SearchMatch, SearchQuery, SearchResult,
    SearchScope, SearchStats, SemanticSearchEngine, SymbolLocation, TypeHierarchyDirection,
    TypeLocation,
};
pub use stubs::StubLoader;
pub use ty::{LiteralValue, Param, ParamKind, Type, TypeVarId, Variance};
pub use type_env::TypeEnv;
pub use typeshed::{TypeshedCache, TypeshedConfig};

pub use propagation::{
    PropagatedType, PropagationPipeline, PropagationRequest, PropagationResult, PropagationStats,
};
