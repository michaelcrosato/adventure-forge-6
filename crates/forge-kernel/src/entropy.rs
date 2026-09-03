use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// Version the algorithm so a future random algorithm can never silently
/// change the meaning of an old trace.
pub const ENTROPY_ALGORITHM_VERSION: &str = "splitmix64-v1";

/// The cursor is deliberately finite.  A trace that reaches the end must
/// fail explicitly instead of wrapping back to the first random value.
pub const MAX_ENTROPY_CURSOR: u64 = u64::MAX - 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntropyError {
    UnsupportedAlgorithm { expected: String, actual: String },
    CursorExhausted,
}

impl Display for EntropyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedAlgorithm { expected, actual } => {
                write!(
                    f,
                    "unsupported entropy algorithm: expected {expected}, got {actual}"
                )
            }
            Self::CursorExhausted => f.write_str("entropy cursor exhausted"),
        }
    }
}

impl std::error::Error for EntropyError {}

/// Explicit, serializable entropy position.  It is copied into GameState and
/// is also accepted by `step`, which makes the source of every random result
/// visible to a replay verifier.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct EntropyState {
    pub algorithm: String,
    pub seed: u64,
    pub cursor: u64,
}

impl EntropyState {
    pub fn new(seed: u64) -> Self {
        Self {
            algorithm: ENTROPY_ALGORITHM_VERSION.to_owned(),
            seed,
            cursor: 0,
        }
    }

    pub fn validate(&self) -> Result<(), EntropyError> {
        if self.algorithm != ENTROPY_ALGORITHM_VERSION {
            return Err(EntropyError::UnsupportedAlgorithm {
                expected: ENTROPY_ALGORITHM_VERSION.to_owned(),
                actual: self.algorithm.clone(),
            });
        }
        if self.cursor > MAX_ENTROPY_CURSOR {
            return Err(EntropyError::CursorExhausted);
        }
        Ok(())
    }

    /// Return the next deterministic draw without mutating the input.
    pub fn next_u64(&self) -> Result<(Self, u64), EntropyError> {
        self.validate()?;
        if self.cursor == MAX_ENTROPY_CURSOR {
            return Err(EntropyError::CursorExhausted);
        }
        let input = self
            .seed
            .wrapping_add(self.cursor.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let value = splitmix64(input);
        let mut next = self.clone();
        next.cursor += 1;
        Ok((next, value))
    }

    pub fn draw(&self) -> Result<EntropyDraw, EntropyError> {
        let (after, value) = self.next_u64()?;
        Ok(EntropyDraw {
            before: self.clone(),
            value,
            after,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EntropyDraw {
    pub before: EntropyState,
    pub value: u64,
    pub after: EntropyState,
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_vectors_are_stable() {
        let expected = [
            0xbdd7_3226_2feb_6e95,
            0x28ef_e333_b266_f103,
            0x4752_6757_130f_9f52,
            0x581c_e1ff_0e4a_e394,
        ];
        let mut state = EntropyState::new(42);
        for expected_value in expected {
            let (next, value) = state.next_u64().expect("known entropy vector is valid");
            assert_eq!(value, expected_value);
            state = next;
        }
        assert_eq!(state.cursor, 4);
    }

    #[test]
    fn unsupported_algorithm_is_rejected_without_a_draw() {
        let state = EntropyState {
            algorithm: "not-splitmix".to_owned(),
            seed: 42,
            cursor: 0,
        };
        let error = state.next_u64().unwrap_err();
        assert_eq!(
            error,
            EntropyError::UnsupportedAlgorithm {
                expected: ENTROPY_ALGORITHM_VERSION.to_owned(),
                actual: "not-splitmix".to_owned(),
            }
        );
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn cursor_exhaustion_is_explicit_and_does_not_wrap() {
        let state = EntropyState {
            algorithm: ENTROPY_ALGORITHM_VERSION.to_owned(),
            seed: 42,
            cursor: MAX_ENTROPY_CURSOR,
        };
        assert_eq!(state.next_u64(), Err(EntropyError::CursorExhausted));
        assert_eq!(state.draw(), Err(EntropyError::CursorExhausted));
        assert_eq!(state.cursor, MAX_ENTROPY_CURSOR);

        let invalid = EntropyState {
            cursor: u64::MAX,
            ..state
        };
        assert_eq!(invalid.validate(), Err(EntropyError::CursorExhausted));
    }
}
