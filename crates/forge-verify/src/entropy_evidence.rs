use crate::VerifyError;
use forge_kernel::{EventKind, GameState};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct EntropyExpectation {
    pub turn: u64,
    pub algorithm: &'static str,
    pub cursor: u64,
    pub value: u64,
}

pub(super) fn validate_entropy_expectations(
    expected: &[EntropyExpectation],
    final_time: Option<u64>,
) -> Result<(), VerifyError> {
    let mut previous_turn = 0;
    for (index, draw) in expected.iter().enumerate() {
        let cursor = u64::try_from(index)
            .map_err(|_| VerifyError::new("scenario entropy expectation count exceeds u64"))?;
        if draw.algorithm != "splitmix64-v1"
            || draw.cursor != cursor
            || draw.turn < previous_turn
            || final_time.is_some_and(|time| draw.turn > time)
        {
            return Err(VerifyError::new(
                "scenario entropy expectations need the reviewed algorithm and ordered turns/cursors",
            ));
        }
        previous_turn = draw.turn;
    }
    Ok(())
}

pub(super) fn validate_entropy_history(
    state: &GameState,
    expected: &[EntropyExpectation],
) -> Result<(), VerifyError> {
    validate_entropy_expectations(expected, Some(state.world.time))?;
    let mut actual = state
        .event_log
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RandomDraw {
                algorithm,
                cursor,
                value,
            } => Some((event.turn, algorithm.as_str(), *cursor, *value)),
            _ => None,
        });
    for draw in expected {
        if actual.next() != Some((draw.turn, draw.algorithm, draw.cursor, draw.value)) {
            return Err(VerifyError::new(
                "scenario random draw history differs from literal entropy answers",
            ));
        }
    }
    let count = u64::try_from(expected.len())
        .map_err(|_| VerifyError::new("scenario entropy expectation count exceeds u64"))?;
    if actual.next().is_some()
        || state.entropy.algorithm != "splitmix64-v1"
        || state.entropy.cursor != count
    {
        return Err(VerifyError::new(
            "scenario entropy history has extra draws or a different final cursor/algorithm",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_kernel::{Event, sha256_json};
    use forge_replay::Session;

    const ANSWERS: &[EntropyExpectation] = &[
        EntropyExpectation {
            turn: 8,
            algorithm: "splitmix64-v1",
            cursor: 0,
            value: 0x974e_3532_5981_068a,
        },
        EntropyExpectation {
            turn: 9,
            algorithm: "splitmix64-v1",
            cursor: 1,
            value: 0x974e_3532_5981_068b,
        },
    ];

    fn oracle_fixture() -> GameState {
        // This is a malformed-state oracle fixture, never submitted as a
        // production transition or evidence of a reachable second rack.
        let content = crate::load_content().unwrap();
        let session = Session::new_game("rook", 27, &content).unwrap();
        let mut state = session.state().clone();
        state.world.time = 10;
        state.entropy.cursor = 2;
        state.event_log = ANSWERS
            .iter()
            .map(|draw| Event {
                turn: draw.turn,
                kind: EventKind::RandomDraw {
                    algorithm: draw.algorithm.to_owned(),
                    cursor: draw.cursor,
                    value: draw.value,
                },
            })
            .collect();
        state
    }

    #[test]
    fn literal_entropy_oracle_rejects_every_field_order_and_count_mutation() {
        let original = oracle_fixture();
        validate_entropy_history(&original, ANSWERS).unwrap();
        for mutation in [
            "turn",
            "algorithm",
            "cursor",
            "value",
            "order",
            "missing",
            "duplicate",
            "unlogged",
            "state algorithm",
        ] {
            let mut state = original.clone();
            match mutation {
                "turn" => state.event_log[0].turn += 1,
                "algorithm" | "cursor" | "value" => {
                    let EventKind::RandomDraw {
                        algorithm,
                        cursor,
                        value,
                    } = &mut state.event_log[0].kind
                    else {
                        unreachable!()
                    };
                    match mutation {
                        "algorithm" => *algorithm = "unreviewed".to_owned(),
                        "cursor" => *cursor += 1,
                        "value" => *value += 1,
                        _ => unreachable!(),
                    }
                }
                "order" => state.event_log.swap(0, 1),
                "missing" => {
                    state.event_log.remove(0);
                }
                "duplicate" => state.event_log.push(state.event_log[0].clone()),
                "unlogged" => state.entropy.cursor += 1,
                "state algorithm" => state.entropy.algorithm = "unreviewed".to_owned(),
                _ => unreachable!(),
            }
            assert!(
                validate_entropy_history(&state, ANSWERS).is_err(),
                "accepted {mutation}"
            );
        }
    }

    #[test]
    fn empty_expectations_require_no_draws_and_an_untouched_cursor() {
        let content = crate::load_content().unwrap();
        let session = Session::new_game("rook", 27, &content).unwrap();
        validate_entropy_history(session.state(), &[]).unwrap();
        let mut hidden_draw = session.state().clone();
        hidden_draw.entropy.cursor = 1;
        assert!(validate_entropy_history(&hidden_draw, &[]).is_err());
        assert!(validate_entropy_history(&oracle_fixture(), &[]).is_err());
    }

    #[test]
    fn entropy_expectations_require_contiguous_cursors_and_chronological_times() {
        validate_entropy_expectations(ANSWERS, Some(10)).unwrap();
        for mutation in [
            "algorithm",
            "initial cursor",
            "cursor gap",
            "time order",
            "future time",
        ] {
            let mut bad = ANSWERS.to_vec();
            match mutation {
                "algorithm" => bad[0].algorithm = "unreviewed",
                "initial cursor" => bad[0].cursor = 1,
                "cursor gap" => bad[1].cursor = 2,
                "time order" => bad[1].turn = 7,
                "future time" => bad[1].turn = 11,
                _ => unreachable!(),
            }
            assert!(
                validate_entropy_expectations(&bad, Some(10)).is_err(),
                "accepted {mutation}"
            );
        }
    }

    #[test]
    fn serialized_entropy_claim_changes_with_each_literal_field() {
        let original = sha256_json(&ANSWERS).unwrap();
        for mutation in ["turn", "algorithm", "cursor", "value", "count", "order"] {
            let mut bad = ANSWERS.to_vec();
            match mutation {
                "turn" => bad[0].turn += 1,
                "algorithm" => bad[0].algorithm = "unreviewed",
                "cursor" => bad[0].cursor += 1,
                "value" => bad[0].value += 1,
                "count" => {
                    bad.pop();
                }
                "order" => bad.swap(0, 1),
                _ => unreachable!(),
            }
            assert_ne!(original, sha256_json(&bad).unwrap(), "unbound {mutation}");
        }
    }
}
