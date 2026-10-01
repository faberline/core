use std::collections::HashSet;

use crate::domain::search::index::SearchIndex;
use crate::type_inference::{SearchQuery, SearchResult};

use super::{entry_in_scope, entry_to_match, make_stats};

// -- 1. ByTypeSignature --

fn parse_type_pattern(p: &str) -> (Vec<String>, Option<String>) {
    let p = p.trim();
    if let Some((params, ret)) = p.split_once("->") {
        let ps = parse_params(params.trim());
        let r = ret.trim().to_lowercase();
        (ps, if r.is_empty() { None } else { Some(r) })
    } else {
        (parse_params(p), None)
    }
}

fn parse_params(s: &str) -> Vec<String> {
    let s = s.trim().trim_start_matches('(').trim_end_matches(')');
    if s.is_empty() {
        return Vec::new();
    }
    s.split(',')
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .collect()
}

pub fn search_by_type_signature(index: &SearchIndex, q: &SearchQuery, pat: &str) -> SearchResult {
    let (qp, qr) = parse_type_pattern(pat);
    let start = std::time::Instant::now();
    let mut matches = Vec::new();
    let mut fs = HashSet::new();
    for e in index.query_by_type(pat) {
        if !entry_in_scope(e, &q.scope) {
            continue;
        }
        fs.insert(e.file.clone());
        let sig = match &e.type_signature {
            Some(s) => s.to_lowercase(),
            None => continue,
        };
        let score = sig_score(&qp, &qr, &sig);
        if score > 0.0 {
            let mut m = entry_to_match(e, score);
            m.symbol = Some(sig);
            matches.push(m);
        }
    }
    matches.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    matches.truncate(q.max_results);
    let total = matches.len();
    SearchResult {
        matches,
        total_count: total,
        stats: make_stats(&fs, start),
    }
}

fn sig_score(qp: &[String], qr: &Option<String>, sig: &str) -> f64 {
    let (sp, sr) = parse_type_pattern(sig);
    let mut score = 0.0_f64;
    let mut n = 1_usize;
    if qp.len() == sp.len() {
        score += 1.0;
    } else if !qp.is_empty() {
        return 0.0;
    }
    for (a, b) in qp.iter().zip(sp.iter()) {
        if a == b {
            score += 1.0;
        } else if b.contains(a.as_str()) {
            score += 0.5;
        }
        n += 1;
    }
    if let Some(ref qret) = qr {
        if let Some(ref sret) = sr {
            if qret == sret {
                score += 1.5;
            } else if sret.contains(qret.as_str()) {
                score += 0.7;
            }
        }
        n += 1;
    }
    if n == 0 {
        0.0
    } else {
        score / n as f64
    }
}
