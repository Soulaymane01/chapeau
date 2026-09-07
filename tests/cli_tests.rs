use chapeau::errors::{ChapeauError, Result};
use std::str::FromStr;

#[test]
fn test_error_display() {
    let err = ChapeauError::ResourceNotFound("python3".to_string());
    assert_eq!(err.to_string(), "resource not found: python3");
}

#[test]
fn test_resource_type_roundtrip() {
    use chapeau::core::ResourceType;

    let cases = vec![
        ResourceType::Package,
        ResourceType::Flatpak,
        ResourceType::Service,
        ResourceType::Repository,
    ];

    for rt in cases {
        let s = rt.as_str();
        let back = ResourceType::from_str(s).unwrap();
        assert_eq!(rt, back);
    }
}

#[test]
fn test_relationship_type_roundtrip() {
    use chapeau::core::RelationshipType;

    let cases = vec![
        RelationshipType::DependsOn,
        RelationshipType::Provides,
        RelationshipType::Owns,
        RelationshipType::Uses,
        RelationshipType::ComesFrom,
    ];

    for rt in cases {
        let s = rt.as_str();
        let back = RelationshipType::from_str(s).unwrap();
        assert_eq!(rt, back);
    }
}
