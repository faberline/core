use super::*;

#[test]
fn test_borrow_state_basic() {
    let mut state = BorrowState::new();
    let lt = LifetimeId(0);

    // Should succeed: first immutable borrow
    let result = state.borrow("x".to_string(), lt, false, (0, 1));
    assert!(result.is_ok());

    // Should succeed: second immutable borrow
    let result = state.borrow("x".to_string(), lt, false, (2, 3));
    assert!(result.is_ok());
}

#[test]
fn test_borrow_state_mutable_conflict() {
    let mut state = BorrowState::new();
    let lt = LifetimeId(0);

    // First mutable borrow
    let result = state.borrow("x".to_string(), lt, true, (0, 1));
    assert!(result.is_ok());

    // Second borrow should fail (mutable + any)
    let result = state.borrow("x".to_string(), lt, false, (2, 3));
    assert!(result.is_err());
}

#[test]
fn test_borrow_state_immutable_then_mutable() {
    let mut state = BorrowState::new();
    let lt = LifetimeId(0);

    // First immutable borrow
    let result = state.borrow("x".to_string(), lt, false, (0, 1));
    assert!(result.is_ok());

    // Mutable borrow should fail
    let result = state.borrow("x".to_string(), lt, true, (2, 3));
    assert!(result.is_err());
}

#[test]
fn test_borrow_state_move() {
    let mut state = BorrowState::new();

    // Move should succeed
    let result = state.move_value("x", (0, 1));
    assert!(result.is_ok());

    // Value should no longer be valid
    assert!(!state.is_valid("x"));

    // Borrow after move should fail
    let lt = LifetimeId(0);
    let result = state.borrow("x".to_string(), lt, false, (2, 3));
    assert!(result.is_err());
}

#[test]
fn test_borrow_state_move_while_borrowed() {
    let mut state = BorrowState::new();
    let lt = LifetimeId(0);

    // Borrow first
    let result = state.borrow("x".to_string(), lt, false, (0, 1));
    assert!(result.is_ok());

    // Move should fail
    let result = state.move_value("x", (2, 3));
    assert!(result.is_err());
}

#[test]
fn test_lifetime_analyzer_basic() {
    let mut analyzer = LifetimeAnalyzer::new();

    let lt1 = analyzer.fresh_lifetime();
    let lt2 = analyzer.fresh_lifetime();

    // Add constraint: lt1 outlives lt2
    analyzer.add_outlives_constraint(lt1, lt2, None);

    // Should validate successfully
    assert!(analyzer.validate_constraints());
    assert!(!analyzer.has_errors());
}

#[test]
fn test_lifetime_analyzer_use_after_move() {
    let mut analyzer = LifetimeAnalyzer::new();

    // Move value
    analyzer.move_value("x", (0, 1));

    // Use after move should error
    analyzer.check_use("x", (2, 3));

    assert!(analyzer.has_errors());
    assert_eq!(analyzer.errors()[0].kind, LifetimeErrorKind::UseAfterMove);
}

#[test]
fn test_lifetime_analyzer_scope() {
    let mut analyzer = LifetimeAnalyzer::new();
    let lt = analyzer.fresh_lifetime();

    // Borrow in outer scope
    let outer = analyzer.enter_scope();
    let id = analyzer.borrow("x".to_string(), lt, false, (0, 1));
    assert!(id.is_some());

    // Exit scope
    analyzer.exit_scope(outer);

    // In new scope, should be able to mutably borrow
    let id = analyzer.borrow("x".to_string(), lt, true, (2, 3));
    assert!(id.is_some());
}
