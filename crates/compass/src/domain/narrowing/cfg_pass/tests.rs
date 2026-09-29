use super::*;
use crate::domain::narrowing::resolution::{resolve_overload, resolve_typevar_bindings};
use crate::domain::type_system::ty::{Param, ParamKind, Type, TypeVarId, Variance};
use crate::semantic::pdg::cfg::CfgBuilder;
use crate::syntax::{Language, MultiParser};

#[test]
fn test_cfg_narrowing_isinstance() {
    let source = "if isinstance(x, int):\n    y = x + 1\nz = x";
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Python).unwrap();
    let cfg = CfgBuilder::new(source).build(&parsed);

    let mut original_types = HashMap::new();
    original_types.insert("x".to_string(), Type::Union(vec![Type::Int, Type::Str]));

    let pass = CfgNarrowingPass::new(&cfg, original_types, source);
    let result = pass.run();

    // Should have computed block environments
    assert!(!result.block_envs.is_empty());
}

#[test]
fn test_cfg_narrowing_is_none() {
    let source = "if x is None:\n    y = 1\nz = x";
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Python).unwrap();
    let cfg = CfgBuilder::new(source).build(&parsed);

    let mut original_types = HashMap::new();
    original_types.insert("x".to_string(), Type::Optional(Box::new(Type::Int)));

    let pass = CfgNarrowingPass::new(&cfg, original_types, source);
    let result = pass.run();

    assert!(!result.block_envs.is_empty());
}

#[test]
fn test_resolve_typevar() {
    // T is a TypeVar, param: list[T], arg: list[int] → T = int
    let t_id = TypeVarId(0);
    let t = Type::TypeVar {
        id: t_id,
        name: "T".to_string(),
        bound: None,
        constraints: vec![],
        variance: Variance::Invariant,
    };

    let param_ty = Type::List(Box::new(t));
    let arg_ty = Type::List(Box::new(Type::Int));

    let bindings = resolve_typevar_bindings(&[param_ty], &[arg_ty]);
    assert_eq!(bindings.get(&t_id), Some(&Type::Int));
}

#[test]
fn test_overload_resolution() {
    // def f(x: int) -> str / def f(x: str) -> int
    let overloaded = Type::Overloaded {
        signatures: vec![
            Type::Callable {
                params: vec![Param {
                    name: "x".to_string(),
                    ty: Type::Int,
                    has_default: false,
                    kind: ParamKind::Positional,
                }],
                ret: Box::new(Type::Str),
            },
            Type::Callable {
                params: vec![Param {
                    name: "x".to_string(),
                    ty: Type::Str,
                    has_default: false,
                    kind: ParamKind::Positional,
                }],
                ret: Box::new(Type::Int),
            },
        ],
    };

    // f(42) should resolve to str
    let result = resolve_overload(&overloaded, &[Type::Int]);
    assert_eq!(result, Some(Type::Str));

    // f("hello") should resolve to int
    let result = resolve_overload(&overloaded, &[Type::Str]);
    assert_eq!(result, Some(Type::Int));
}
