//! Literal storage claims, independent of reducer execution and content stock.
//! The scenario binds the entire final store map and every directional record.

use crate::VerifyError;
use forge_kernel::{EventKind, GameState};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct StorageBalanceExpectation {
    pub storage: &'static str,
    pub inventory: &'static [(&'static str, u32)],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StorageTransferDirection {
    ToCharacter,
    FromCharacter,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct StorageTransferExpectation {
    pub turn: u64,
    pub direction: StorageTransferDirection,
    pub storage: &'static str,
    pub item: &'static str,
    pub count: u32,
}

pub(super) fn validate_storage_expectations(
    balances: &[StorageBalanceExpectation],
    transfers: &[StorageTransferExpectation],
    final_time: Option<u64>,
) -> Result<(), VerifyError> {
    let mut storages = BTreeSet::new();
    for balance in balances {
        if balance.storage.trim().is_empty() || !storages.insert(balance.storage) {
            return Err(VerifyError::new(
                "scenario storage balances require unique named stores",
            ));
        }
        let mut items = BTreeSet::new();
        for (item, count) in balance.inventory {
            if item.trim().is_empty() || *count == 0 || !items.insert(*item) {
                return Err(VerifyError::new(
                    "scenario storage inventory requires unique named items and positive counts",
                ));
            }
        }
    }
    let mut previous_turn = 0;
    for transfer in transfers {
        if !storages.contains(transfer.storage)
            || transfer.item.trim().is_empty()
            || transfer.count == 0
            || transfer.turn < previous_turn
            || final_time.is_some_and(|time| transfer.turn > time)
        {
            return Err(VerifyError::new(
                "scenario storage transfers require declared stores, named items, positive counts, and chronological turns",
            ));
        }
        previous_turn = transfer.turn;
    }
    Ok(())
}

pub(super) fn validate_storage_history(
    state: &GameState,
    balances: &[StorageBalanceExpectation],
    transfers: &[StorageTransferExpectation],
) -> Result<(), VerifyError> {
    validate_storage_expectations(balances, transfers, Some(state.world.time))?;
    if state.world.storages.len() != balances.len() {
        return Err(VerifyError::new(
            "scenario final storage registry differs from literal balances",
        ));
    }
    for balance in balances {
        let Some(actual) = state.world.storages.get(balance.storage) else {
            return Err(VerifyError::new(
                "scenario final storage is missing a literal owner",
            ));
        };
        if actual.inventory.len() != balance.inventory.len()
            || balance
                .inventory
                .iter()
                .any(|(item, count)| actual.inventory.get(*item) != Some(count))
        {
            return Err(VerifyError::new(
                "scenario final storage inventory differs from literal balances",
            ));
        }
    }
    // Never filter by a known storage ID: an invented owner or otherwise
    // unexpected transfer must remain visible to the exact history oracle.
    let mut actual = state
        .event_log
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::StorageItemTransferredToCharacter {
                storage,
                item,
                count,
            } => Some((
                event.turn,
                StorageTransferDirection::ToCharacter,
                storage.as_str(),
                item.as_str(),
                *count,
            )),
            EventKind::CharacterItemTransferredToStorage {
                storage,
                item,
                count,
            } => Some((
                event.turn,
                StorageTransferDirection::FromCharacter,
                storage.as_str(),
                item.as_str(),
                *count,
            )),
            _ => None,
        });
    for transfer in transfers {
        if actual.next()
            != Some((
                transfer.turn,
                transfer.direction,
                transfer.storage,
                transfer.item,
                transfer.count,
            ))
        {
            return Err(VerifyError::new(
                "scenario storage transfer order or fields differ from literal history",
            ));
        }
    }
    if actual.next().is_some() {
        return Err(VerifyError::new(
            "scenario storage history contains unexpected transfers",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_kernel::{Event, StorageState, sha256_json};
    use std::collections::BTreeMap;

    const BALANCES: &[StorageBalanceExpectation] = &[
        StorageBalanceExpectation {
            storage: "fixture.cage",
            inventory: &[("fixture.fuel", 1)],
        },
        StorageBalanceExpectation {
            storage: "fixture.bin",
            inventory: &[],
        },
    ];
    const TRANSFERS: &[StorageTransferExpectation] = &[
        StorageTransferExpectation {
            turn: 8,
            direction: StorageTransferDirection::FromCharacter,
            storage: "fixture.cage",
            item: "fixture.fuel",
            count: 1,
        },
        StorageTransferExpectation {
            turn: 8,
            direction: StorageTransferDirection::ToCharacter,
            storage: "fixture.cage",
            item: "fixture.filter",
            count: 1,
        },
    ];

    fn oracle_fixture() -> GameState {
        // Deliberately handwritten independent actual-side fixture. Neither
        // side is produced by executing the reducer or reading production.
        serde_json::from_value(serde_json::json!({
            "build_id": "fixture.build",
            "world": {
                "id": "fixture.world", "time": 9, "current_location": "fixture.room",
                "locations": {}, "npcs": {}, "flags": [], "scheduled_events": [],
                "storages": {
                    "fixture.cage": {"inventory": {"fixture.fuel": 1}},
                    "fixture.bin": {"inventory": {}}
                }
            },
            "character": {
                "id": "fixture.player", "lineage": "fixture.lineage", "origin": "fixture.origin", "background": "fixture.work",
                "aptitudes": {}, "skills": [], "values": [], "traits": [], "flaws": [], "appearance": {},
                "affiliations": {}, "reputation": {}, "knowledge": [], "inventory": {}, "resources": {},
                "injuries": [], "deeds": [], "promises": [], "discoveries": [], "facets": {}
            },
            "character_start": {"kind": "fixture"},
            "entropy": {"algorithm": "splitmix64-v1", "seed": 71, "cursor": 0},
            "event_log": [
                {"turn": 8, "kind": {"kind": "character_item_transferred_to_storage", "storage": "fixture.cage", "item": "fixture.fuel", "count": 1}},
                {"turn": 8, "kind": {"kind": "flag_set", "flag": "fixture.settled", "value": true}},
                {"turn": 8, "kind": {"kind": "storage_item_transferred_to_character", "storage": "fixture.cage", "item": "fixture.filter", "count": 1}}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn literal_storage_oracle_rejects_every_transfer_field_order_and_count_mutation() {
        let original = oracle_fixture();
        validate_storage_history(&original, BALANCES, TRANSFERS).unwrap();
        for mutation in [
            "direction",
            "storage",
            "item",
            "count",
            "turn",
            "order",
            "missing",
            "extra",
            "unknown extra",
        ] {
            let mut state = original.clone();
            match mutation {
                "direction" => {
                    state.event_log[0].kind = EventKind::StorageItemTransferredToCharacter {
                        storage: "fixture.cage".to_owned(),
                        item: "fixture.fuel".to_owned(),
                        count: 1,
                    };
                }
                "storage" | "item" | "count" => {
                    let EventKind::CharacterItemTransferredToStorage {
                        storage,
                        item,
                        count,
                    } = &mut state.event_log[0].kind
                    else {
                        unreachable!()
                    };
                    match mutation {
                        "storage" => *storage = "fixture.bin".to_owned(),
                        "item" => *item = "fixture.filter".to_owned(),
                        "count" => *count = 2,
                        _ => unreachable!(),
                    }
                }
                "turn" => state.event_log[0].turn = 7,
                // Equal turns prove that literal order is checked separately
                // from the schema's chronological ordering requirement.
                "order" => state.event_log.swap(0, 2),
                "missing" => {
                    state.event_log.remove(0);
                }
                "extra" => state.event_log.push(state.event_log[0].clone()),
                "unknown extra" => state.event_log.push(Event {
                    turn: 9,
                    kind: EventKind::StorageItemTransferredToCharacter {
                        storage: "fixture.unknown".to_owned(),
                        item: "fixture.filter".to_owned(),
                        count: 1,
                    },
                }),
                _ => unreachable!(),
            }
            assert!(
                validate_storage_history(&state, BALANCES, TRANSFERS).is_err(),
                "accepted transfer mutation {mutation}"
            );
        }
        let mut wrong_time = original;
        wrong_time.world.time = 7;
        assert!(validate_storage_history(&wrong_time, BALANCES, TRANSFERS).is_err());
    }

    #[test]
    fn literal_storage_oracle_requires_full_owner_maps_and_exact_reserve_quantities() {
        let original = oracle_fixture();
        for mutation in [
            "count",
            "extra item",
            "missing item",
            "missing store",
            "unknown store",
            "zero entry",
            "reserve",
        ] {
            let mut state = original.clone();
            match mutation {
                "count" => {
                    state
                        .world
                        .storages
                        .get_mut("fixture.cage")
                        .unwrap()
                        .inventory
                        .insert("fixture.fuel".to_owned(), 2);
                }
                "extra item" => {
                    state
                        .world
                        .storages
                        .get_mut("fixture.bin")
                        .unwrap()
                        .inventory
                        .insert("fixture.filter".to_owned(), 1);
                }
                "missing item" => state
                    .world
                    .storages
                    .get_mut("fixture.cage")
                    .unwrap()
                    .inventory
                    .clear(),
                "missing store" => {
                    state.world.storages.remove("fixture.bin");
                }
                "unknown store" => {
                    state.world.storages.remove("fixture.bin");
                    state
                        .world
                        .storages
                        .insert("fixture.other".to_owned(), StorageState::default());
                }
                "zero entry" => {
                    state
                        .world
                        .storages
                        .get_mut("fixture.bin")
                        .unwrap()
                        .inventory
                        .insert("fixture.fuel".to_owned(), 0);
                }
                "reserve" => {
                    state
                        .world
                        .storages
                        .get_mut("fixture.cage")
                        .unwrap()
                        .inventory
                        .insert("fixture.filter".to_owned(), 1);
                }
                _ => unreachable!(),
            }
            assert!(
                validate_storage_history(&state, BALANCES, TRANSFERS).is_err(),
                "accepted final balance mutation {mutation}"
            );
        }
        assert!(validate_storage_history(&original, &BALANCES[..1], TRANSFERS).is_err());
    }

    #[test]
    fn storage_spec_rejects_ambiguous_balances_and_invalid_transfer_claims() {
        validate_storage_expectations(BALANCES, TRANSFERS, Some(9)).unwrap();
        for invalid in [
            StorageBalanceExpectation {
                storage: " ",
                inventory: &[],
            },
            StorageBalanceExpectation {
                storage: "fixture.cage",
                inventory: &[("", 1)],
            },
            StorageBalanceExpectation {
                storage: "fixture.cage",
                inventory: &[("fixture.fuel", 0)],
            },
            StorageBalanceExpectation {
                storage: "fixture.cage",
                inventory: &[("fixture.fuel", 1), ("fixture.fuel", 2)],
            },
        ] {
            assert!(validate_storage_expectations(&[invalid], &[], Some(9)).is_err());
        }
        assert!(validate_storage_expectations(&[BALANCES[0], BALANCES[0]], &[], Some(9)).is_err());
        for invalid in [
            StorageTransferExpectation {
                storage: " ",
                ..TRANSFERS[0]
            },
            StorageTransferExpectation {
                storage: "fixture.unknown",
                ..TRANSFERS[0]
            },
            StorageTransferExpectation {
                item: " ",
                ..TRANSFERS[0]
            },
            StorageTransferExpectation {
                count: 0,
                ..TRANSFERS[0]
            },
            StorageTransferExpectation {
                turn: 10,
                ..TRANSFERS[0]
            },
        ] {
            assert!(validate_storage_expectations(BALANCES, &[invalid], Some(9)).is_err());
        }
        let backwards = StorageTransferExpectation {
            turn: 7,
            ..TRANSFERS[1]
        };
        assert!(
            validate_storage_expectations(BALANCES, &[TRANSFERS[0], backwards], Some(9)).is_err()
        );
        let zero_turn = StorageTransferExpectation {
            turn: 0,
            ..TRANSFERS[0]
        };
        validate_storage_expectations(BALANCES, &[zero_turn], Some(0)).unwrap();
    }

    #[test]
    fn empty_transfer_claim_preserves_untouched_stock_and_rejects_hidden_activity() {
        const UNTOUCHED: &[StorageBalanceExpectation] = &[
            StorageBalanceExpectation {
                storage: "fixture.cage",
                inventory: &[("fixture.filter", 1)],
            },
            StorageBalanceExpectation {
                storage: "fixture.bin",
                inventory: &[],
            },
        ];
        let mut state = oracle_fixture();
        state
            .world
            .storages
            .get_mut("fixture.cage")
            .unwrap()
            .inventory = BTreeMap::from([("fixture.filter".to_owned(), 1)]);
        state.event_log.clear();
        validate_storage_history(&state, UNTOUCHED, &[]).unwrap();
        state.event_log.push(Event {
            turn: 8,
            kind: EventKind::StorageItemTransferredToCharacter {
                storage: "fixture.cage".to_owned(),
                item: "fixture.filter".to_owned(),
                count: 1,
            },
        });
        state.event_log.push(Event {
            turn: 8,
            kind: EventKind::CharacterItemTransferredToStorage {
                storage: "fixture.cage".to_owned(),
                item: "fixture.filter".to_owned(),
                count: 1,
            },
        });
        assert!(
            validate_storage_history(&state, UNTOUCHED, &[]).is_err(),
            "net-zero stock changes still require literal transfer history"
        );
    }

    #[test]
    fn storage_claim_serialization_binds_balances_and_directional_history() {
        let identity = sha256_json(&(BALANCES, TRANSFERS)).unwrap();
        let changed_direction = [
            StorageTransferExpectation {
                direction: StorageTransferDirection::ToCharacter,
                ..TRANSFERS[0]
            },
            TRANSFERS[1],
        ];
        assert_ne!(
            identity,
            sha256_json(&(BALANCES, changed_direction)).unwrap()
        );
        let changed_stock = [
            StorageBalanceExpectation {
                storage: "fixture.cage",
                inventory: &[("fixture.fuel", 2)],
            },
            BALANCES[1],
        ];
        assert_ne!(identity, sha256_json(&(changed_stock, TRANSFERS)).unwrap());
    }
}
