use crate::VerifyError;
use forge_kernel::{EventKind, GameState};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

// The pre-existing absolute event has its own unchanged deadline predicates.
// Every other scheduled/resolved identity belongs in this explicit oracle;
// unknown IDs cannot disappear merely because content lacks their definition.
const ABSOLUTE_SURGE: &str = "lowsail.next_surge";

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum DeferredEventExpectation {
    Scheduled {
        turn: u64,
        event_id: &'static str,
        event_kind: &'static str,
        due_time: u64,
    },
    Resolved {
        turn: u64,
        event_id: &'static str,
        event_kind: &'static str,
        applied: bool,
    },
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct PendingEventExpectation {
    pub event_id: &'static str,
    pub event_kind: &'static str,
    pub due_time: u64,
}

pub(super) fn validate_deferred_expectations(
    events: &[DeferredEventExpectation],
    pending: &[PendingEventExpectation],
    final_time: Option<u64>,
) -> Result<(), VerifyError> {
    let mut used = BTreeSet::new();
    let mut queue = BTreeMap::new();
    let mut last_turn = 0;
    for event in events {
        let (turn, id, kind) = match event {
            DeferredEventExpectation::Scheduled {
                turn,
                event_id,
                event_kind,
                due_time,
            } => {
                if *due_time <= *turn || !used.insert(*event_id) {
                    return Err(VerifyError::new(
                        "scenario deferred schedule must be positive and once-only",
                    ));
                }
                queue.insert(*event_id, (*event_kind, *due_time));
                (*turn, *event_id, *event_kind)
            }
            DeferredEventExpectation::Resolved {
                turn,
                event_id,
                event_kind,
                ..
            } => {
                let next = queue.iter().min_by_key(|(id, (_, due))| (*due, **id));
                if !next.is_some_and(|(id, (kind, due))| {
                    *id == *event_id && *kind == *event_kind && *due <= *turn
                }) {
                    return Err(VerifyError::new(
                        "scenario deferred resolution lacks its exact schedule",
                    ));
                }
                queue.remove(event_id);
                (*turn, *event_id, *event_kind)
            }
        };
        if id.trim().is_empty()
            || id == ABSOLUTE_SURGE
            || kind.trim().is_empty()
            || turn < last_turn
            || final_time.is_some_and(|end| turn > end)
        {
            return Err(VerifyError::new(
                "scenario deferred history needs named, chronological records",
            ));
        }
        last_turn = turn;
    }
    let mut unresolved: Vec<_> = queue
        .into_iter()
        .map(|(id, (kind, due))| (due, id, kind))
        .collect();
    unresolved.sort_unstable();
    if unresolved.len() != pending.len()
        || unresolved
            .iter()
            .zip(pending)
            .any(|((due, id, kind), expected)| {
                *due != expected.due_time
                    || *id != expected.event_id
                    || *kind != expected.event_kind
                    || final_time.is_some_and(|end| *due <= end)
            })
    {
        return Err(VerifyError::new(
            "scenario pending deferred records must equal the unresolved future queue",
        ));
    }
    Ok(())
}

pub(super) fn validate_deferred_history(
    state: &GameState,
    expected: &[DeferredEventExpectation],
    pending: &[PendingEventExpectation],
) -> Result<(), VerifyError> {
    let actual: Vec<_> = state
        .event_log
        .iter()
        .filter(|event| match &event.kind {
            EventKind::EventScheduled { .. } => true,
            EventKind::ScheduledEventResolved { event_id, .. } => event_id != ABSOLUTE_SURGE,
            _ => false,
        })
        .collect();
    if actual.len() != expected.len() {
        return Err(VerifyError::new(
            "scenario deferred event history length differs",
        ));
    }
    for (actual, expected) in actual.into_iter().zip(expected) {
        let matches = match (&actual.kind, expected) {
            (
                EventKind::EventScheduled {
                    event_id,
                    event_kind,
                    due_time,
                },
                DeferredEventExpectation::Scheduled {
                    turn,
                    event_id: id,
                    event_kind: kind,
                    due_time: due,
                },
            ) => actual.turn == *turn && event_id == id && event_kind == kind && due_time == due,
            (
                EventKind::ScheduledEventResolved {
                    event_id,
                    event_kind,
                    applied,
                },
                DeferredEventExpectation::Resolved {
                    turn,
                    event_id: id,
                    event_kind: kind,
                    applied: applies,
                },
            ) => actual.turn == *turn && event_id == id && event_kind == kind && applied == applies,
            _ => false,
        };
        if !matches {
            return Err(VerifyError::new(
                "scenario deferred event order or fields differ",
            ));
        }
    }
    let actual_pending: Vec<_> = state
        .world
        .scheduled_events
        .iter()
        .filter(|event| event.id != ABSOLUTE_SURGE)
        .collect();
    if actual_pending.len() != pending.len() {
        return Err(VerifyError::new(
            "scenario pending deferred event count differs",
        ));
    }
    for (actual, expected) in actual_pending.into_iter().zip(pending) {
        if actual.id != expected.event_id
            || actual.event_kind != expected.event_kind
            || actual.due_time != expected.due_time
        {
            return Err(VerifyError::new(
                "scenario pending deferred event order or fields differ",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deferred_oracle_rejects_impossible_claimed_history_and_pending_queue() {
        let scheduled = DeferredEventExpectation::Scheduled {
            turn: 14,
            event_id: "fixture.ready",
            event_kind: "production",
            due_time: 16,
        };
        let resolved = DeferredEventExpectation::Resolved {
            turn: 16,
            event_id: "fixture.ready",
            event_kind: "production",
            applied: true,
        };
        let pending = PendingEventExpectation {
            event_id: "fixture.ready",
            event_kind: "production",
            due_time: 16,
        };
        assert!(validate_deferred_expectations(&[scheduled], &[pending], Some(15)).is_ok());
        assert!(validate_deferred_expectations(&[scheduled, resolved], &[], Some(16)).is_ok());
        assert!(validate_deferred_expectations(&[scheduled], &[], Some(15)).is_err());
        assert!(validate_deferred_expectations(&[scheduled], &[pending], Some(16)).is_err());
        assert!(validate_deferred_expectations(&[resolved], &[], Some(16)).is_err());
        assert!(
            validate_deferred_expectations(&[scheduled, scheduled], &[pending], Some(15)).is_err()
        );
        assert!(
            validate_deferred_expectations(&[scheduled, resolved], &[pending], Some(16)).is_err()
        );
        let early = DeferredEventExpectation::Resolved {
            turn: 15,
            event_id: "fixture.ready",
            event_kind: "production",
            applied: true,
        };
        assert!(validate_deferred_expectations(&[scheduled, early], &[], Some(16)).is_err());
        assert!(validate_deferred_expectations(&[scheduled, resolved], &[], Some(15)).is_err());
    }

    #[test]
    fn deferred_oracle_orders_tied_deadlines_and_allows_crossed_resolution() {
        let a = DeferredEventExpectation::Scheduled {
            turn: 0,
            event_id: "fixture.a",
            event_kind: "production",
            due_time: 2,
        };
        let b = DeferredEventExpectation::Scheduled {
            turn: 0,
            event_id: "fixture.b",
            event_kind: "production",
            due_time: 2,
        };
        let resolved = |event_id| DeferredEventExpectation::Resolved {
            turn: 3,
            event_id,
            event_kind: "production",
            applied: true,
        };
        assert!(
            validate_deferred_expectations(
                &[b, a, resolved("fixture.a"), resolved("fixture.b")],
                &[],
                Some(3),
            )
            .is_ok()
        );
        assert!(
            validate_deferred_expectations(
                &[a, b, resolved("fixture.b"), resolved("fixture.a")],
                &[],
                Some(3),
            )
            .is_err()
        );
    }
}
