//! Positional pattern filtering with 3-tier fallback (exact → starts-with → contains).
//!
//! All `list` commands accept zero or more positional patterns that filter by
//! the primary name of each item. Zero patterns means "all". With one or more
//! patterns, matching is tried in three tiers in order, and the first tier
//! with any match wins (OR semantics within a tier):
//!
//! 1. **exact** - any pattern equals the item's name
//! 2. **starts-with** - any pattern is a prefix of the item's name
//! 3. **contains** - any pattern is a substring of the item's name
//!
//! This mirrors the `gx` / `aka` filtering convention.

/// Filter `items` by `patterns` using 3-tier fallback matching.
/// `name_of` extracts the name to match against for each item.
pub fn filter<'a, T, F>(items: &'a [T], patterns: &[String], name_of: F) -> Vec<&'a T>
where
    F: Fn(&T) -> &str,
{
    if patterns.is_empty() {
        return items.iter().collect();
    }

    let t1: Vec<&T> = items
        .iter()
        .filter(|i| patterns.iter().any(|p| name_of(i) == p))
        .collect();
    if !t1.is_empty() {
        return t1;
    }

    let t2: Vec<&T> = items
        .iter()
        .filter(|i| patterns.iter().any(|p| name_of(i).starts_with(p)))
        .collect();
    if !t2.is_empty() {
        return t2;
    }

    items
        .iter()
        .filter(|i| patterns.iter().any(|p| name_of(i).contains(p)))
        .collect()
}

/// Owned variant of `filter`: takes items by value and returns owned matches.
/// Useful when filtering `Vec<Value>` from deserialization where cloning is cheap.
pub fn filter_into<T, F>(items: Vec<T>, patterns: &[String], name_of: F) -> Vec<T>
where
    F: Fn(&T) -> &str,
{
    if patterns.is_empty() {
        return items;
    }

    let mut t1 = Vec::new();
    let mut t2 = Vec::new();
    let mut t3 = Vec::new();

    for item in items {
        let name = name_of(&item);
        if patterns.iter().any(|p| name == p) {
            t1.push(item);
        } else if patterns.iter().any(|p| name.starts_with(p)) {
            t2.push(item);
        } else if patterns.iter().any(|p| name.contains(p)) {
            t3.push(item);
        }
    }

    if !t1.is_empty() {
        return t1;
    }
    if !t2.is_empty() {
        return t2;
    }
    t3
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Item {
        name: String,
    }

    impl Item {
        fn new(name: &str) -> Self {
            Self { name: name.to_string() }
        }
    }

    fn items(names: &[&str]) -> Vec<Item> {
        names.iter().map(|n| Item::new(n)).collect()
    }

    fn pats(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn empty_patterns_returns_all() {
        let data = items(&["alpha", "beta", "gamma"]);
        let result = filter(&data, &[], |i| &i.name);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn exact_match_wins_over_prefix_or_contains() {
        let data = items(&["Platform", "Platform Primary", "Data Platform"]);
        let result = filter(&data, &pats(&["Platform"]), |i| &i.name);
        // Only the exact match should be returned; prefix/contains are skipped.
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Platform");
    }

    #[test]
    fn starts_with_wins_when_no_exact_match() {
        let data = items(&["Platform Primary", "Data Platform", "Infrastructure"]);
        let result = filter(&data, &pats(&["Platform"]), |i| &i.name);
        // Only "Platform Primary" starts with "Platform"; "Data Platform" contains it.
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Platform Primary");
    }

    #[test]
    fn contains_is_used_only_when_no_exact_or_prefix_match() {
        let data = items(&["Data Platform", "Infrastructure Platform"]);
        let result = filter(&data, &pats(&["Platform"]), |i| &i.name);
        // No exact, no prefix; contains matches both.
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn multiple_patterns_union_within_a_tier() {
        let data = items(&["alpha", "beta", "gamma", "delta"]);
        let result = filter(&data, &pats(&["alpha", "gamma"]), |i| &i.name);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "alpha");
        assert_eq!(result[1].name, "gamma");
    }

    #[test]
    fn multiple_patterns_stop_at_first_matching_tier() {
        // "alpha" is exact; "plat" would be contains on "Platform".
        // Tier 1 match on "alpha" wins; "plat" never looks at tier 3.
        let data = items(&["alpha", "beta", "Platform"]);
        let result = filter(&data, &pats(&["alpha", "plat"]), |i| &i.name);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "alpha");
    }

    #[test]
    fn no_match_returns_empty() {
        let data = items(&["alpha", "beta"]);
        let result = filter(&data, &pats(&["xyz"]), |i| &i.name);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_into_matches_filter_semantics() {
        let data = items(&["Platform", "Platform Primary", "Data Platform"]);
        let result = filter_into(data, &pats(&["Platform"]), |i| &i.name);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Platform");
    }

    #[test]
    fn filter_into_empty_patterns_returns_all() {
        let data = items(&["alpha", "beta"]);
        let result = filter_into(data, &[], |i| &i.name);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn case_sensitive_by_default() {
        // Design doc is silent on case; default to case-sensitive to match gx/aka.
        let data = items(&["Platform"]);
        let result = filter(&data, &pats(&["platform"]), |i| &i.name);
        // No exact/starts-with match; "platform" does not "contains" "Platform" (case-sensitive).
        assert!(result.is_empty());
    }
}
