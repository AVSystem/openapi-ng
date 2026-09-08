//! Duplicate-rejecting map deserializers.
//!
//! YAML and JSON both permit a repeated mapping key, which `BTreeMap` and
//! `IndexMap` resolve by keeping the last occurrence.

use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;

use indexmap::IndexMap;
use serde::de::{Deserialize, Deserializer, Error as _, MapAccess, Visitor};

/// A keyed collection that reports whether a key was already present.
pub(crate) trait InsertUnique {
  type Value;

  /// Inserts `value` under `key`. Returns `false` and leaves the collection
  /// unchanged when `key` was already present.
  fn insert_unique(&mut self, key: String, value: Self::Value) -> bool;
}

impl<V> InsertUnique for BTreeMap<String, V> {
  type Value = V;

  fn insert_unique(&mut self, key: String, value: V) -> bool {
    match self.entry(key) {
      std::collections::btree_map::Entry::Occupied(_) => false,
      std::collections::btree_map::Entry::Vacant(slot) => {
        slot.insert(value);
        true
      }
    }
  }
}

impl<V> InsertUnique for IndexMap<String, V> {
  type Value = V;

  fn insert_unique(&mut self, key: String, value: V) -> bool {
    match self.entry(key) {
      indexmap::map::Entry::Occupied(_) => false,
      indexmap::map::Entry::Vacant(slot) => {
        slot.insert(value);
        true
      }
    }
  }
}

/// Wraps a keyed collection so deserializing it rejects a repeated key.
/// Derefs to the wrapped collection.
#[derive(Debug, Default)]
#[cfg_attr(test, derive(Clone))]
pub(crate) struct Unique<M>(M);

/// `BTreeMap` that rejects a repeated key at decode time.
pub(crate) type UniqueMap<V> = Unique<BTreeMap<String, V>>;

/// `IndexMap` that rejects a repeated key at decode time, preserving the
/// spec author's declaration order.
pub(crate) type UniqueIndexMap<V> = Unique<IndexMap<String, V>>;

/// Total: a built `BTreeMap` or `IndexMap` holds no repeated key, so only
/// deserialization can encounter one.
impl<M> From<M> for Unique<M> {
  fn from(map: M) -> Self {
    Self(map)
  }
}

impl<M> Deref for Unique<M> {
  type Target = M;

  fn deref(&self) -> &M {
    &self.0
  }
}

impl<'de, M> Deserialize<'de> for Unique<M>
where
  M: InsertUnique + Default,
  M::Value: Deserialize<'de>,
{
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer.deserialize_map(UniqueVisitor(PhantomData))
  }
}

struct UniqueVisitor<M>(PhantomData<M>);

impl<'de, M> Visitor<'de> for UniqueVisitor<M>
where
  M: InsertUnique + Default,
  M::Value: Deserialize<'de>,
{
  type Value = Unique<M>;

  fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("a map")
  }

  fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
    let mut map = M::default();
    while let Some(key) = access.next_key::<String>()? {
      let value = access.next_value::<M::Value>()?;
      if !map.insert_unique(key.clone(), value) {
        return Err(A::Error::custom(format!("{DUPLICATE_KEY} '{key}'")));
      }
    }
    Ok(Unique(map))
  }
}

/// Leading text of the duplicate-key error, for callers routing on it.
pub(crate) const DUPLICATE_KEY: &str = "duplicate key";

#[cfg(test)]
mod tests {
  use super::{UniqueIndexMap, UniqueMap};

  #[test]
  fn unique_keys_deserialize_into_the_wrapped_map() {
    let map: UniqueMap<u8> = serde_yml::from_str("a: 1\nb: 2\n").expect("unique keys parse");
    assert_eq!(map.len(), 2);
    assert_eq!(map.get("a"), Some(&1));
  }

  #[test]
  fn repeated_key_fails_naming_the_key() {
    let error =
      serde_yml::from_str::<UniqueMap<u8>>("a: 1\nb: 2\na: 3\n").expect_err("repeat must fail");
    assert!(error.to_string().contains("duplicate key 'a'"));
  }

  /// The field path and source position come from the deserializer, so they
  /// appear once the map sits under a named field — which is every use in
  /// the OpenAPI model.
  #[test]
  fn repeated_key_under_a_field_reports_the_path_and_position() {
    #[derive(Debug, serde::Deserialize)]
    struct Doc {
      #[allow(dead_code)]
      schemas: UniqueMap<u8>,
    }

    let error =
      serde_yml::from_str::<Doc>("schemas:\n  a: 1\n  a: 2\n").expect_err("repeat must fail");
    let message = error.to_string();
    assert!(message.contains("schemas: duplicate key 'a'"), "{message}");
    assert!(
      message.contains("line ") && message.contains("column "),
      "{message}"
    );
  }

  #[test]
  fn index_map_variant_preserves_declaration_order() {
    let map: UniqueIndexMap<u8> =
      serde_yml::from_str("z: 1\na: 2\n").expect("unique keys parse in order");
    assert_eq!(
      map.keys().map(String::as_str).collect::<Vec<_>>(),
      ["z", "a"]
    );
  }

  #[test]
  fn index_map_variant_rejects_a_repeated_key() {
    let error =
      serde_yml::from_str::<UniqueIndexMap<u8>>("z: 1\nz: 2\n").expect_err("repeat must fail");
    assert!(error.to_string().contains("duplicate key 'z'"));
  }
}
