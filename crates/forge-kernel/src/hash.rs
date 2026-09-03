use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
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
