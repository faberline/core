use super::*;

#[test]
fn pool_round_robins_across_connections() {
    let pool = H2cPool::with_connections(3).unwrap();
    assert_eq!(pool.connections(), 3);
    let ptr = |c: &reqwest::Client| c as *const reqwest::Client;
    let a = ptr(pool.client());
    let b = ptr(pool.client());
    let c = ptr(pool.client());
    let d = ptr(pool.client());
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
    assert_eq!(a, d); // wraps back to the first connection
}

#[test]
fn pool_floor_is_one_connection() {
    assert_eq!(H2cPool::with_connections(0).unwrap().connections(), 1);
}
