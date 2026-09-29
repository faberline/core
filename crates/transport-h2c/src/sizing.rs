/// Recommended number of h2c connections for a target peak `concurrency`, using
/// the available CPU parallelism as the upper cap.
///
/// See the crate docs for the rationale. Equivalent to
/// [`recommended_h2c_connections_for`] with `parallelism = available cores`.
pub fn recommended_h2c_connections(concurrency: usize) -> usize {
    recommended_h2c_connections_for(concurrency, cpu_parallelism())
}

/// Like [`recommended_h2c_connections`] but with an explicit core cap, for
/// deterministic sizing and testing.
///
/// `connections = clamp(ceil(ln(concurrency)), 1, parallelism)`.
pub fn recommended_h2c_connections_for(concurrency: usize, parallelism: usize) -> usize {
    let cap = parallelism.max(1);
    if concurrency <= 2 {
        return 1;
    }
    let ln = (concurrency as f64).ln().ceil() as usize;
    ln.clamp(1, cap)
}

/// Available CPU parallelism (`std::thread::available_parallelism`), or 1.
pub fn cpu_parallelism() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests;
