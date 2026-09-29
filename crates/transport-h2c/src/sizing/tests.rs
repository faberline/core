use super::*;

#[test]
fn heuristic_is_log_shaped() {
    // ceil(ln(c)), capped well above so the log shape shows through
    assert_eq!(recommended_h2c_connections_for(16, 64), 3); // ln=2.77
    assert_eq!(recommended_h2c_connections_for(64, 64), 5); // ln=4.16
    assert_eq!(recommended_h2c_connections_for(256, 64), 6); // ln=5.55
    assert_eq!(recommended_h2c_connections_for(1024, 64), 7); // ln=6.93
    assert_eq!(recommended_h2c_connections_for(4096, 64), 9); // ln=8.32
}

#[test]
fn heuristic_clamps_to_cores_and_floor() {
    // never exceeds the core cap
    assert_eq!(recommended_h2c_connections_for(1_000_000, 4), 4);
    // tiny concurrency → a single connection
    assert_eq!(recommended_h2c_connections_for(0, 8), 1);
    assert_eq!(recommended_h2c_connections_for(1, 8), 1);
    assert_eq!(recommended_h2c_connections_for(2, 8), 1);
    // core cap is at least 1 even if passed 0
    assert_eq!(recommended_h2c_connections_for(1024, 0), 1);
}
