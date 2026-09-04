use serde::Serialize;
use serde::de::{Error as DeError, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};

/// Errors produced while turning a serializable value into canonical JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashError(pub String);

impl Display for HashError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for HashError {}

/// Serialize a value to deterministic JSON bytes.
///
/// Objects are rebuilt in lexicographic key order.  The kernel's data types
/// use integers rather than floats, so serde_json's normal number formatting
/// is stable across supported platforms.
pub fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, HashError> {
    let value = serde_json::to_value(value)
        .map_err(|error| HashError(format!("cannot serialize for hashing: {error}")))?;
    let value = canonicalize(value);
    serde_json::to_vec(&value)
        .map_err(|error| HashError(format!("cannot encode canonical JSON: {error}")))
}

fn canonicalize(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries: Vec<_> = object.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = Map::new();
            for (key, value) in entries {
                sorted.insert(key, canonicalize(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize).collect()),
        other => other,
    }
}

pub fn sha256_hex_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

pub fn sha256_json<T: Serialize>(value: &T) -> Result<String, HashError> {
    Ok(sha256_hex_bytes(&canonical_json_bytes(value)?))
}

/// Reject ambiguous JSON before typed deserialization can collapse duplicate
/// object keys. This walks the entire document, including nested maps.
pub fn validate_unique_json_keys(input: &str) -> Result<(), HashError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    serde::de::Deserializer::deserialize_any(&mut deserializer, UniqueJsonVisitor)
        .map_err(|error| HashError(format!("invalid or ambiguous JSON: {error}")))?;
    deserializer
        .end()
        .map_err(|error| HashError(format!("invalid or ambiguous JSON: {error}")))
}

struct UniqueJsonVisitor;

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    fn visit_bool<E: DeError>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i64<E: DeError>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u64<E: DeError>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_f64<E: DeError>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_str<E: DeError>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_string<E: DeError>(self, _value: String) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_none<E: DeError>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_unit<E: DeError>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_some<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        while sequence.next_element_seed(UniqueJsonSeed)?.is_some() {}
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Self::Value, A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = object.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(A::Error::custom(format!("duplicate object key {key}")));
            }
            object.next_value_seed(UniqueJsonSeed)?;
        }
        Ok(())
    }
}

struct UniqueJsonSeed;

impl<'de> serde::de::DeserializeSeed<'de> for UniqueJsonSeed {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::validate_unique_json_keys;

    #[test]
    fn duplicate_json_keys_are_rejected_at_every_depth() {
        assert!(validate_unique_json_keys(r#"{"a":1,"b":[{"c":2}]}"#).is_ok());
        assert!(validate_unique_json_keys(r#"{"a":1,"a":2}"#).is_err());
        assert!(validate_unique_json_keys(r#"{"a":{"b":1,"b":2}}"#).is_err());
    }
}
