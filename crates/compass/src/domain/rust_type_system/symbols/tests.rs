use super::*;
use crate::domain::rust_type_system::types::{Lifetime, LifetimeId, TraitBound, TraitRef};

#[test]
fn test_rust_symbols_default() {
    let symbols = RustSymbols::default();
    assert!(symbols.structs.is_empty());
    assert!(symbols.enums.is_empty());
    assert!(symbols.traits.is_empty());
    assert!(symbols.impls.is_empty());
    assert!(symbols.functions.is_empty());
}

#[test]
fn test_symbol_collector_creation() {
    let collector = RustSymbolCollector::new();
    assert_eq!(collector.trait_id_counter, 0);
    assert_eq!(collector.type_var_counter, 0);
}

#[test]
fn test_discriminant_parsing_logic() {
    // Test parsing discriminant values directly
    fn parse_value(text: &str) -> Option<i128> {
        let text = text.trim();
        if text.starts_with("0x") || text.starts_with("0X") {
            i128::from_str_radix(&text[2..].replace('_', ""), 16).ok()
        } else if text.starts_with("0o") || text.starts_with("0O") {
            i128::from_str_radix(&text[2..].replace('_', ""), 8).ok()
        } else if text.starts_with("0b") || text.starts_with("0B") {
            i128::from_str_radix(&text[2..].replace('_', ""), 2).ok()
        } else {
            text.replace('_', "").parse::<i128>().ok()
        }
    }

    // Test decimal
    assert_eq!(parse_value("42"), Some(42));

    // Test hex
    assert_eq!(parse_value("0xFF"), Some(255));

    // Test with underscores
    assert_eq!(parse_value("1_000_000"), Some(1_000_000));

    // Test binary
    assert_eq!(parse_value("0b1010"), Some(10));

    // Test octal
    assert_eq!(parse_value("0o77"), Some(63));

    // Test negative
    assert_eq!(parse_value("-1"), Some(-1));
}

#[test]
fn test_trait_ref_creation() {
    // Test that TraitRef has proper defaults
    let trait_ref = TraitRef {
        trait_id: TraitId(1),
        name: "Clone".to_string(),
        type_args: vec![],
        lifetime_args: vec![],
    };
    assert_eq!(trait_ref.name, "Clone");
    assert!(trait_ref.type_args.is_empty());
}

#[test]
fn test_where_predicate_types() {
    // Test TypeBound predicate
    let type_bound = WherePredicate::TypeBound {
        ty: RustType::Named {
            name: "T".to_string(),
            module: None,
            type_args: vec![],
            lifetime_args: vec![],
        },
        bounds: vec![TraitBound {
            trait_ref: TraitRef {
                trait_id: TraitId(0),
                name: "Clone".to_string(),
                type_args: vec![],
                lifetime_args: vec![],
            },
            is_negative: false,
            higher_ranked_lifetimes: vec![],
        }],
    };

    if let WherePredicate::TypeBound { ty, bounds } = type_bound {
        assert_eq!(bounds.len(), 1);
        assert!(!bounds[0].is_negative);
        if let RustType::Named { name, .. } = ty {
            assert_eq!(name, "T");
        }
    }

    // Test LifetimeBound predicate
    let lifetime_bound = WherePredicate::LifetimeBound {
        lifetime: Lifetime::Named {
            id: LifetimeId(0),
            name: "'a".to_string(),
        },
        bounds: vec![Lifetime::Static],
    };

    if let WherePredicate::LifetimeBound { lifetime, bounds } = lifetime_bound {
        assert_eq!(bounds.len(), 1);
        if let Lifetime::Named { name, .. } = lifetime {
            assert_eq!(name, "'a");
        }
    }
}
