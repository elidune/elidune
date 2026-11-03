//! Typo-tolerant cluster key matching and merging.

use std::collections::HashMap;

/// Normalized similarity in `[0, 1]` (1 = identical).
pub fn key_similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let a = a.trim();
    let b = b.trim();
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let dist = levenshtein_chars(a, b);
    let max_len = a.chars().count().max(b.chars().count()) as f64;
    1.0 - (dist as f64 / max_len)
}

/// Merge tiny typo clusters (few biblios) into a larger similar title.
///
/// Avoids chaining merges across the whole catalog (which would collapse unrelated serials).
pub fn suggest_cluster_merges(
    cluster_sizes: &HashMap<String, usize>,
    min_similarity: f64,
) -> HashMap<String, String> {
    const MAX_SMALL_CLUSTER: usize = 3;
    let keys: Vec<String> = cluster_sizes.keys().cloned().collect();
    let mut merges = HashMap::new();
    for small_key in &keys {
        let small_size = cluster_sizes.get(small_key).copied().unwrap_or(0);
        if small_size > MAX_SMALL_CLUSTER {
            continue;
        }
        let mut best: Option<(f64, String)> = None;
        for large_key in &keys {
            if small_key == large_key {
                continue;
            }
            let large_size = cluster_sizes.get(large_key).copied().unwrap_or(0);
            if large_size < small_size {
                continue;
            }
            let sim = key_similarity(small_key, large_key);
            if sim < min_similarity {
                continue;
            }
            if best.as_ref().map(|(score, _)| sim > *score).unwrap_or(true) {
                best = Some((sim, large_key.clone()));
            }
        }
        if let Some((_, canon)) = best {
            if canon != *small_key {
                merges.insert(small_key.clone(), canon);
            }
        }
    }
    compress_merge_chains(merges)
}

fn compress_merge_chains(mut merges: HashMap<String, String>) -> HashMap<String, String> {
    fn resolve(key: &str, merges: &HashMap<String, String>) -> String {
        let mut current = key.to_string();
        let mut seen = std::collections::HashSet::new();
        while let Some(next) = merges.get(&current) {
            if !seen.insert(current.clone()) {
                break;
            }
            current = next.clone();
        }
        current
    }
    merges
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .map(|k| {
            let target = resolve(&k, &merges);
            (k, target)
        })
        .filter(|(k, t)| k != t)
        .collect()
}

pub fn resolve_cluster_key(key: &str, merges: &HashMap<String, String>) -> String {
    let mut current = key.to_string();
    let mut seen = std::collections::HashSet::new();
    while let Some(next) = merges.get(&current) {
        if !seen.insert(current.clone()) {
            break;
        }
        current = next.clone();
    }
    current
}

fn levenshtein_chars(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similar_saisons_typo() {
        assert!(key_similarity("4 saisons", "4 saisonns") >= 0.90);
    }

    #[test]
    fn suggest_merges_typo_into_larger() {
        let sizes = HashMap::from([
            ("4 saisons".to_string(), 31),
            ("4 saisonns".to_string(), 1),
        ]);
        let merges = suggest_cluster_merges(&sizes, 0.90);
        assert_eq!(merges.get("4 saisonns").map(String::as_str), Some("4 saisons"));
    }
}
