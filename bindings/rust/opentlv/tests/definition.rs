// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

use opentlv::{Definition, DefinitionRegistry, Tag};

#[test]
fn generic_registry_keeps_storage_and_c_first_match_semantics() {
    let registry = DefinitionRegistry::new([
        Definition::new(Tag::from_bytes(&[1]))
            .named("first-?")
            .unwrap(),
        Definition::new(Tag::from_bytes(&[1]))
            .named("second")
            .unwrap(),
        Definition::new(Tag::from_bytes(&[])),
    ]);
    let registry = Box::new(registry);
    assert_eq!(
        registry
            .find(&Tag::from_bytes(&[1]))
            .unwrap()
            .name
            .as_ref()
            .unwrap()
            .to_str()
            .unwrap(),
        "first-?"
    );
    assert!(registry.find(&Tag::from_bytes(&[])).unwrap().name.is_none());
    assert!(registry.find(&Tag::from_bytes(&[2])).is_none());
    assert_eq!(registry.definitions().len(), 3);
    assert!(DefinitionRegistry::new([])
        .find(&Tag::from_bytes(&[]))
        .is_none());
    assert!(Definition::new(Tag::from_bytes(&[1]))
        .named("bad\0name")
        .is_err());
}
