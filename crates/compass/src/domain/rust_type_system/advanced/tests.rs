use super::*;

// -----------------------------------------------------------------------
// Array size expressions
// -----------------------------------------------------------------------

#[test]
fn test_literal_array_size() {
    let env = HashMap::new();
    let expr = ArraySizeExpr::Literal(16);
    assert_eq!(expr.evaluate(&env), Some(16));
}

#[test]
fn test_const_param_array_size() {
    let mut env = HashMap::new();
    env.insert("N".to_string(), 8_usize);
    let expr = ArraySizeExpr::ConstParam("N".to_string());
    assert_eq!(expr.evaluate(&env), Some(8));
}

#[test]
fn test_binop_add() {
    let mut env = HashMap::new();
    env.insert("N".to_string(), 3_usize);
    env.insert("M".to_string(), 5_usize);
    let expr = ArraySizeExpr::BinOp {
        op: SizeOp::Add,
        lhs: Box::new(ArraySizeExpr::ConstParam("N".to_string())),
        rhs: Box::new(ArraySizeExpr::ConstParam("M".to_string())),
    };
    assert_eq!(expr.evaluate(&env), Some(8));
}

#[test]
fn test_binop_mul() {
    let mut env = HashMap::new();
    env.insert("N".to_string(), 4_usize);
    let expr = ArraySizeExpr::BinOp {
        op: SizeOp::Mul,
        lhs: Box::new(ArraySizeExpr::ConstParam("N".to_string())),
        rhs: Box::new(ArraySizeExpr::Literal(2)),
    };
    assert_eq!(expr.evaluate(&env), Some(8));
}

#[test]
fn test_binop_div_by_zero() {
    let env = HashMap::new();
    let expr = ArraySizeExpr::BinOp {
        op: SizeOp::Div,
        lhs: Box::new(ArraySizeExpr::Literal(10)),
        rhs: Box::new(ArraySizeExpr::Literal(0)),
    };
    assert_eq!(expr.evaluate(&env), None);
}

#[test]
fn test_missing_const_param() {
    let env = HashMap::new();
    let expr = ArraySizeExpr::ConstParam("UNKNOWN".to_string());
    assert_eq!(expr.evaluate(&env), None);
}

#[test]
fn test_into_array_type() {
    let env = HashMap::new();
    let expr = ArraySizeExpr::Literal(4);
    let ty = expr.into_array_type(RustType::U8, &env);
    assert_eq!(
        ty,
        RustType::Array {
            element: Box::new(RustType::U8),
            size: 4,
        }
    );
}

// -----------------------------------------------------------------------
// Lifetime elision
// -----------------------------------------------------------------------

#[test]
fn test_elision_rule1_no_output() {
    let mut counter = 0_usize;
    let result = apply_lifetime_elision(false, 2, false, &mut counter);
    assert_eq!(result.rule, ElisionRule::EachInputGetsOwn);
    assert_eq!(result.input_lifetimes.len(), 2);
    assert!(result.output_lifetime.is_none());
    assert_eq!(counter, 2);
}

#[test]
fn test_elision_rule2_single_input() {
    let mut counter = 0_usize;
    // fn foo<'a>(x: &'a str) -> &str  — rule 2 applies
    let result = apply_lifetime_elision(false, 1, true, &mut counter);
    assert_eq!(result.rule, ElisionRule::SingleInputToOutput);
    assert!(result.output_lifetime.is_some());
    assert_eq!(result.input_lifetimes.len(), 1);
}

#[test]
fn test_elision_rule3_self_ref() {
    let mut counter = 0_usize;
    // fn method(&self, x: &str) -> &str  — rule 3 applies
    let result = apply_lifetime_elision(true, 2, true, &mut counter);
    assert_eq!(result.rule, ElisionRule::SelfToOutput);
    assert!(result.output_lifetime.is_some());
}

#[test]
fn test_elision_no_elided_lifetimes() {
    let mut counter = 0_usize;
    // fn foo(x: i32) -> i32  — no lifetimes at all
    let result = apply_lifetime_elision(false, 0, false, &mut counter);
    assert_eq!(result.input_lifetimes.len(), 0);
    assert!(result.output_lifetime.is_none());
}

// -----------------------------------------------------------------------
// RustAdvancedInferencer integration
// -----------------------------------------------------------------------

#[test]
fn test_advanced_inferencer_array_size() {
    let ctx = RustTypeContext::new();
    let inf = RustAdvancedInferencer::new(&ctx);
    let expr = ArraySizeExpr::Literal(32);
    assert_eq!(inf.evaluate_array_size(&expr, &HashMap::new()), Some(32));
}

#[test]
fn test_advanced_inferencer_elision() {
    let ctx = RustTypeContext::new();
    let mut inf = RustAdvancedInferencer::new(&ctx);
    let result = inf.apply_elision(false, 1, true);
    assert_eq!(result.rule, ElisionRule::SingleInputToOutput);
}
