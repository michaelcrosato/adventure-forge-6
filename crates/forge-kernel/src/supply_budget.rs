//! Conservative bounds on the number of simultaneously owned item lines.
//!
//! Recipes connect item types. If every recipe in a connected component
//! consumes at least as many units as it produces, total units across all
//! owners cannot exceed their authored genesis bound. A displayed item line
//! needs at least one unit. Taking the most expensive possible lines up to
//! that bound is sound even when those particular items cannot coexist.
//! Growing components retain the original full-union bound.

use crate::{ContentDraft, Effect};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn potential_item_words(draft: &ContentDraft, items: &BTreeSet<String>) -> usize {
    item_words_with_closure(draft, items, None)
}

/// Refine the established conservation bound only after a finite, deliberately
/// permissive inventory model has been exhausted. A partial search is never a
/// proof and cannot lower the fallback.
pub(super) fn tighter_potential_item_words(
    draft: &ContentDraft,
    items: &BTreeSet<String>,
) -> usize {
    let fallback = potential_item_words(draft, items);
    fallback.min(item_words_with_closure(
        draft,
        items,
        Some(ClosureBudget::default()),
    ))
}

#[derive(Clone, Copy)]
struct ClosureBudget {
    states: usize,
    recipe_attempts: usize,
}

impl Default for ClosureBudget {
    fn default() -> Self {
        Self {
            states: 4_096,
            recipe_attempts: 65_536,
        }
    }
}

fn item_words_with_closure(
    draft: &ContentDraft,
    items: &BTreeSet<String>,
    closure_budget: Option<ClosureBudget>,
) -> usize {
    let line_words = |item: &String| {
        draft
            .supply_labels
            .items
            .get(item)
            .unwrap_or(item)
            .split_whitespace()
            .count()
            .saturating_add(1)
    };
    if !known_inventory_operations(draft) {
        return items.iter().map(line_words).fold(0, usize::saturating_add);
    }
    let mut adjacent: BTreeMap<String, BTreeSet<String>> = items
        .iter()
        .map(|item| (item.clone(), BTreeSet::new()))
        .collect();
    for recipe in &draft.recipes {
        let Some(first) = recipe.inputs.keys().next() else {
            // Invalid recipes are rejected before budgeting. A conservative
            // fallback also keeps this helper safe on incomplete authoring.
            return items.iter().map(line_words).fold(0, usize::saturating_add);
        };
        for item in recipe.inputs.keys().chain(recipe.outputs.keys()) {
            adjacent
                .entry(first.clone())
                .or_default()
                .insert(item.clone());
            adjacent
                .entry(item.clone())
                .or_default()
                .insert(first.clone());
        }
    }
    let mut visited = BTreeSet::new();
    let mut total = 0usize;
    for first in adjacent.keys() {
        if visited.contains(first) {
            continue;
        }
        let mut component = BTreeSet::new();
        let mut pending = vec![first.clone()];
        while let Some(item) = pending.pop() {
            if !component.insert(item.clone()) {
                continue;
            }
            pending.extend(adjacent[&item].iter().cloned());
        }
        visited.extend(component.iter().cloned());
        let recipe_component = draft
            .recipes
            .iter()
            .any(|recipe| recipe.inputs.keys().any(|item| component.contains(item)));
        let conserved = recipe_component
            && draft.recipes.iter().all(|recipe| {
                if !recipe.inputs.keys().any(|item| component.contains(item)) {
                    return true;
                }
                let sum = |counts: &BTreeMap<String, u32>| {
                    counts
                        .values()
                        .try_fold(0u64, |total, count| total.checked_add(u64::from(*count)))
                };
                matches!((sum(&recipe.inputs), sum(&recipe.outputs)),
                (Some(inputs), Some(outputs)) if outputs <= inputs)
            });
        let slots = if conserved {
            genesis_units(draft, &component).min(component.len())
        } else {
            component.len()
        };
        let mut costs: Vec<_> = component.iter().map(line_words).collect();
        costs.sort_unstable_by(|left, right| right.cmp(left));
        let fallback = costs.into_iter().take(slots).fold(0, usize::saturating_add);
        let refined = if conserved {
            closure_budget
                .and_then(|budget| closed_component_words(draft, &component, line_words, budget))
        } else {
            None
        };
        total = total.saturating_add(refined.map_or(fallback, |words| words.min(fallback)));
    }
    total
}

fn inventory_units(inventory: &BTreeMap<String, u32>, component: &BTreeSet<String>) -> usize {
    inventory
        .iter()
        .filter(|(item, _)| component.contains(*item))
        .map(|(_, count)| usize::try_from(*count).unwrap_or(usize::MAX))
        .fold(0, usize::saturating_add)
}

fn genesis_units(draft: &ContentDraft, component: &BTreeSet<String>) -> usize {
    let stock = draft
        .npcs
        .iter()
        .map(|npc| inventory_units(&npc.inventory, component))
        .chain(
            draft
                .storages
                .iter()
                .map(|storage| inventory_units(&storage.inventory, component)),
        )
        .fold(0, usize::saturating_add);
    let preset = draft
        .character_presets
        .iter()
        .map(|preset| inventory_units(&preset.character.inventory, component))
        .max()
        .unwrap_or_default();
    // A character chooses one entry per slot. Sum per-slot maxima; this is
    // conservative even if the choices attaining those maxima conflict.
    let custom = draft.character_creation.as_ref().map_or(0, |creation| {
        creation.slots.iter().fold(
            inventory_units(&creation.base.inventory, component),
            |total, slot| {
                total.saturating_add(
                    slot.choices
                        .iter()
                        .map(|choice| inventory_units(&choice.patch.inventory, component))
                        .max()
                        .unwrap_or_default(),
                )
            },
        )
    });
    stock.saturating_add(preset.max(custom))
}

/// Pool every owner's initial stock. Per-item character maxima may combine
/// mutually exclusive starts; that extra stock makes the model conservative.
/// Any real recipe sequence remains applicable from this larger pool, and
/// the surplus stays nonnegative throughout that sequence.
fn pooled_genesis(draft: &ContentDraft, items: &[String]) -> Option<Vec<u64>> {
    items
        .iter()
        .map(|item| {
            let count = |inventory: &BTreeMap<String, u32>| {
                u64::from(inventory.get(item).copied().unwrap_or_default())
            };
            let stock = draft
                .npcs
                .iter()
                .map(|npc| count(&npc.inventory))
                .chain(
                    draft
                        .storages
                        .iter()
                        .map(|storage| count(&storage.inventory)),
                )
                .try_fold(0u64, u64::checked_add)?;
            let preset = draft
                .character_presets
                .iter()
                .map(|preset| count(&preset.character.inventory))
                .max()
                .unwrap_or_default();
            let custom = match &draft.character_creation {
                None => 0,
                Some(creation) => creation.slots.iter().try_fold(
                    count(&creation.base.inventory),
                    |total, slot| {
                        total.checked_add(
                            slot.choices
                                .iter()
                                .map(|choice| count(&choice.patch.inventory))
                                .max()
                                .unwrap_or_default(),
                        )
                    },
                )?,
            };
            stock.checked_add(preset.max(custom))
        })
        .collect()
}

fn closed_component_words(
    draft: &ContentDraft,
    component: &BTreeSet<String>,
    line_words: impl Fn(&String) -> usize,
    budget: ClosureBudget,
) -> Option<usize> {
    let items: Vec<_> = component.iter().cloned().collect();
    let indexes: BTreeMap<_, _> = items
        .iter()
        .enumerate()
        .map(|(index, item)| (item.as_str(), index))
        .collect();
    let recipes: Vec<_> = draft
        .recipes
        .iter()
        .filter(|recipe| recipe.inputs.keys().any(|item| component.contains(item)))
        .map(|recipe| {
            let indexed = |counts: &BTreeMap<String, u32>| {
                counts
                    .iter()
                    .map(|(item, count)| Some((*indexes.get(item.as_str())?, u64::from(*count))))
                    .collect::<Option<Vec<_>>>()
            };
            Some((indexed(&recipe.inputs)?, indexed(&recipe.outputs)?))
        })
        .collect::<Option<_>>()?;
    let initial = pooled_genesis(draft, &items)?;
    let costs: Vec<_> = items.iter().map(line_words).collect();
    let mut seen = BTreeSet::from([initial.clone()]);
    if seen.len() > budget.states {
        return None;
    }
    let mut pending = vec![initial];
    let mut attempts = 0usize;
    let mut maximum = 0usize;
    while let Some(inventory) = pending.pop() {
        let words = inventory
            .iter()
            .zip(&costs)
            .filter(|(count, _)| **count > 0)
            .map(|(_, words)| *words)
            .fold(0usize, usize::saturating_add);
        maximum = maximum.max(words);
        for (inputs, outputs) in &recipes {
            attempts = attempts.checked_add(1)?;
            if attempts > budget.recipe_attempts {
                return None;
            }
            if !inputs
                .iter()
                .all(|(index, count)| inventory[*index] >= *count)
            {
                continue;
            }
            let mut next = inventory.clone();
            for (index, count) in inputs {
                next[*index] = next[*index].checked_sub(*count)?;
            }
            for (index, count) in outputs {
                next[*index] = next[*index].checked_add(*count)?;
            }
            if !seen.contains(&next) {
                if seen.len() >= budget.states {
                    return None;
                }
                seen.insert(next.clone());
                pending.push(next);
            }
        }
    }
    Some(maximum)
}

fn known_inventory_operations(draft: &ContentDraft) -> bool {
    draft
        .actions
        .iter()
        .flat_map(|action| &action.effects)
        .chain(draft.timed_events.iter().flat_map(|event| &event.effects))
        .chain(
            draft
                .deferred_events
                .iter()
                .flat_map(|event| &event.effects),
        )
        .all(known_inventory_effect)
}

fn known_inventory_effect(effect: &Effect) -> bool {
    // Keep this exhaustive: a future item-creating effect must explicitly
    // preserve this proof or force the conservative full-union fallback.
    match effect {
        Effect::RandomChance {
            on_success,
            on_failure,
            ..
        } => known_inventory_effect(on_success) && known_inventory_effect(on_failure),
        Effect::ApplyRecipe { .. }
        | Effect::TransferNpcItemToCharacter { .. }
        | Effect::TransferStorageItemToCharacter { .. }
        | Effect::TransferCharacterItemToStorage { .. }
        | Effect::Noop
        | Effect::SetFlag { .. }
        | Effect::SetWorldFlag { .. }
        | Effect::SetLocationFlag { .. }
        | Effect::AdjustResource { .. }
        | Effect::MoveCharacter { .. }
        | Effect::MoveNpc { .. }
        | Effect::AdjustNpcRelationship { .. }
        | Effect::AddNpcMemory { .. }
        | Effect::TeachNpc { .. }
        | Effect::AddCharacterDeed { .. }
        | Effect::AdvanceTime { .. }
        | Effect::ScheduleEvent { .. } => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CharacterCreationChoice, CharacterCreationDefinition, CharacterCreationSlot,
        CharacterPatch, NpcDefinition, RecipeDefinition,
    };

    fn draft() -> ContentDraft {
        serde_json::from_str(r#"{"schema_version":"forge-schema-v10","rules_version":"forge-rules-v8","world_id":"fixture"}"#).unwrap()
    }

    fn recipe(id: &str, inputs: &[(&str, u32)], outputs: &[(&str, u32)]) -> RecipeDefinition {
        let map =
            |entries: &[(&str, u32)]| entries.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect();
        RecipeDefinition {
            id: id.to_owned(),
            inputs: map(inputs),
            outputs: map(outputs),
        }
    }

    fn stock(draft: &mut ContentDraft, entries: &[(&str, u32)]) {
        draft.npcs.push(NpcDefinition {
            id: format!("owner.{}", draft.npcs.len()),
            name: "Owner".to_owned(),
            location: "room".to_owned(),
            goals: BTreeSet::new(),
            values: BTreeSet::new(),
            tags: BTreeSet::new(),
            inventory: entries.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect(),
        });
    }

    fn items(draft: &ContentDraft) -> BTreeSet<String> {
        draft
            .recipes
            .iter()
            .flat_map(|r| r.inputs.keys().chain(r.outputs.keys()))
            .chain(draft.npcs.iter().flat_map(|npc| npc.inventory.keys()))
            .chain(
                draft
                    .storages
                    .iter()
                    .flat_map(|storage| storage.inventory.keys()),
            )
            .cloned()
            .collect()
    }

    #[test]
    fn conversion_components_bound_lines_by_total_stock_and_largest_labels() {
        let mut draft = draft();
        stock(&mut draft, &[("clay", 2), ("mesh", 1)]);
        stock(&mut draft, &[("fuel", 1), ("rope", 1)]);
        draft.recipes = vec![
            recipe("r.prepare", &[("clay", 2), ("mesh", 1)], &[("charge", 1)]),
            recipe("r.ignite", &[("charge", 1), ("fuel", 1)], &[("claim", 1)]),
            recipe("r.draw", &[("claim", 1)], &[("filter", 1)]),
            recipe("r.spoil", &[("claim", 1)], &[("rejects", 1)]),
            recipe("r.fit", &[("filter", 1)], &[]),
        ];
        draft
            .supply_labels
            .items
            .insert("claim".to_owned(), "Kiln batch ticket".to_owned());
        // Four possible lines in the connected conversion component: one
        // four-word line plus three two-word lines. Rope is independent.
        assert_eq!(potential_item_words(&draft, &items(&draft)), 12);
        // A second custodian's stock is real stock, not another view of the
        // first inventory. It raises the upper bound by two possible lines.
        stock(&mut draft, &[("clay", 2)]);
        assert_eq!(potential_item_words(&draft, &items(&draft)), 16);
    }

    #[test]
    fn any_growing_recipe_keeps_the_entire_connected_union() {
        let mut draft = draft();
        stock(&mut draft, &[("seed", 1)]);
        draft.recipes = vec![
            recipe("r.grow", &[("seed", 1)], &[("seed", 1), ("leaf", 1)]),
            recipe("r.press", &[("leaf", 1)], &[("paper", 1)]),
            recipe("r.fold", &[("paper", 1)], &[("box", 1)]),
        ];
        assert_eq!(potential_item_words(&draft, &items(&draft)), 8);
    }

    #[test]
    fn custom_genesis_includes_base_and_independent_slot_maxima() {
        let mut draft = draft();
        let patch = |count| CharacterPatch {
            inventory: BTreeMap::from([("raw".to_owned(), count)]),
            ..CharacterPatch::default()
        };
        let slot = |id: &str, counts: &[u32]| CharacterCreationSlot {
            id: id.to_owned(),
            display_name: id.to_owned(),
            order: 0,
            choices: counts
                .iter()
                .enumerate()
                .map(|(n, count)| CharacterCreationChoice {
                    id: format!("{id}.{n}"),
                    display_name: "Choice".to_owned(),
                    summary: "Choice".to_owned(),
                    patch: patch(*count),
                })
                .collect(),
        };
        draft.character_creation = Some(CharacterCreationDefinition {
            base: patch(1),
            slots: vec![slot("first", &[1, 2]), slot("second", &[1, 3])],
        });
        draft.recipes = (0..8)
            .map(|n| {
                recipe(
                    &format!("r.{n}"),
                    &[("raw", 1)],
                    &[(&format!("goods{n}"), 1)],
                )
            })
            .collect();
        assert_eq!(potential_item_words(&draft, &items(&draft)), 12);
        stock(&mut draft, &[("raw", 1)]);
        assert_eq!(potential_item_words(&draft, &items(&draft)), 14);
    }

    #[test]
    fn every_reachable_inventory_in_a_finite_conversion_system_fits_the_bound() {
        let mut draft = draft();
        stock(&mut draft, &[("raw", 2), ("binder", 1)]);
        draft.recipes = vec![
            recipe("r.prepare", &[("raw", 1), ("binder", 1)], &[("charge", 1)]),
            recipe("r.draw", &[("charge", 1)], &[("finished", 1)]),
            recipe("r.reclaim", &[("charge", 1)], &[("raw", 1)]),
            recipe("r.install", &[("finished", 1)], &[]),
        ];
        draft
            .supply_labels
            .items
            .insert("finished".to_owned(), "Finished useful goods".to_owned());
        let bound = potential_item_words(&draft, &items(&draft));
        let mut seen = BTreeSet::new();
        let mut pending = vec![draft.npcs[0].inventory.clone()];
        while let Some(inventory) = pending.pop() {
            if !seen.insert(inventory.clone()) {
                continue;
            }
            let actual: usize = inventory
                .keys()
                .map(|item| {
                    draft
                        .supply_labels
                        .items
                        .get(item)
                        .unwrap_or(item)
                        .split_whitespace()
                        .count()
                        + 1
                })
                .sum();
            assert!(
                actual <= bound,
                "reachable owned lines exceeded conserved bound"
            );
            for recipe in &draft.recipes {
                if !recipe
                    .inputs
                    .iter()
                    .all(|(item, count)| inventory.get(item).copied().unwrap_or_default() >= *count)
                {
                    continue;
                }
                let mut next = inventory.clone();
                for (item, count) in &recipe.inputs {
                    let remaining = next[item] - count;
                    if remaining == 0 {
                        next.remove(item);
                    } else {
                        next.insert(item.clone(), remaining);
                    }
                }
                for (item, count) in &recipe.outputs {
                    *next.entry(item.clone()).or_default() += count;
                }
                pending.push(next);
            }
        }
        assert!(seen.len() >= 5);
    }

    #[test]
    fn exhausted_recipe_closure_preserves_literal_multiowner_frontiers() {
        let mut draft = draft();
        stock(&mut draft, &[("clay", 2), ("mesh", 1)]);
        stock(&mut draft, &[("fuel", 1), ("rope", 1)]);
        draft.recipes = vec![
            recipe("prepare", &[("clay", 2), ("mesh", 1)], &[("charge", 1)]),
            recipe("ignite", &[("charge", 1), ("fuel", 1)], &[("claim", 1)]),
            recipe("draw", &[("claim", 1)], &[("filter", 1)]),
            recipe("spoil", &[("claim", 1)], &[("rejects", 1)]),
            recipe("fit", &[("filter", 1)], &[]),
        ];
        draft
            .supply_labels
            .items
            .insert("claim".to_owned(), "Kiln batch ticket".to_owned());
        // Hand-counted frontiers include stock split across custodians: initial
        // clay/mesh/fuel/rope = 8; charge/fuel/rope = 6; claim/rope = 6;
        // filter/rope or rejects/rope = 4; installed filter leaves rope = 2.
        assert_eq!(potential_item_words(&draft, &items(&draft)), 12);
        assert_eq!(tighter_potential_item_words(&draft, &items(&draft)), 8);
        draft.storages.push(crate::StorageDefinition {
            id: "fixed.cage".to_owned(),
            name: "Collateral cage".to_owned(),
            location: "room".to_owned(),
            inventory: BTreeMap::from([("filter".to_owned(), 1)]),
        });
        // The stored spare can coexist with every earlier frontier. It adds
        // one two-word line to the initial maximum; it is never free stock.
        assert_eq!(tighter_potential_item_words(&draft, &items(&draft)), 10);
        assert_eq!(
            pooled_genesis(&draft, &["filter".to_owned()]),
            Some(vec![1])
        );
    }

    #[test]
    fn incomplete_closure_never_uses_a_partial_maximum() {
        let mut draft = draft();
        stock(&mut draft, &[("raw", 1)]);
        draft.recipes = vec![
            recipe("first", &[("raw", 1)], &[("intermediate", 1)]),
            recipe("last", &[("intermediate", 1)], &[("finished", 1)]),
        ];
        draft.supply_labels.items.insert(
            "finished".to_owned(),
            "Seven whole words name these finished goods".to_owned(),
        );
        let all = items(&draft);
        let expected = 8;
        assert_eq!(potential_item_words(&draft, &all), expected);
        for budget in [
            ClosureBudget {
                states: 0,
                recipe_attempts: 100,
            },
            ClosureBudget {
                states: 1,
                recipe_attempts: 100,
            },
            ClosureBudget {
                states: 2,
                recipe_attempts: 100,
            },
            ClosureBudget {
                states: 100,
                recipe_attempts: 0,
            },
            ClosureBudget {
                states: 100,
                recipe_attempts: 1,
            },
        ] {
            assert_eq!(
                item_words_with_closure(&draft, &all, Some(budget)),
                expected
            );
        }
        assert_eq!(tighter_potential_item_words(&draft, &all), expected);
    }

    #[test]
    fn cyclic_consuming_and_unstocked_recipes_have_complete_finite_closures() {
        let mut draft = draft();
        stock(&mut draft, &[("left", 1)]);
        draft.recipes = vec![
            recipe("right", &[("left", 1)], &[("right", 1)]),
            recipe("left", &[("right", 1)], &[("left", 1)]),
            recipe("use", &[("right", 1)], &[]),
        ];
        let component = items(&draft);
        assert_eq!(
            closed_component_words(&draft, &component, |_| 2, ClosureBudget::default()),
            Some(2)
        );
        draft.npcs[0].inventory.clear();
        assert_eq!(
            closed_component_words(&draft, &component, |_| 2, ClosureBudget::default()),
            Some(0)
        );
        assert_eq!(tighter_potential_item_words(&draft, &component), 0);
    }

    #[test]
    fn growing_or_incomplete_recipe_components_keep_the_full_fallback() {
        let mut draft = draft();
        stock(&mut draft, &[("seed", 1)]);
        draft.recipes = vec![
            recipe("grow", &[("seed", 1)], &[("seed", 1), ("leaf", 1)]),
            recipe("fold", &[("leaf", 1)], &[("paper", 1)]),
        ];
        assert_eq!(tighter_potential_item_words(&draft, &items(&draft)), 6);
        // Malformed recipes are rejected by the compiler, but an incomplete
        // authoring draft cannot make this proof silently omit its output.
        draft.recipes.push(recipe("invalid", &[], &[("extra", 1)]));
        assert_eq!(tighter_potential_item_words(&draft, &items(&draft)), 8);
    }

    #[test]
    fn pooled_seed_uses_wide_arithmetic_and_every_character_item_maximum() {
        let mut draft = draft();
        stock(&mut draft, &[("raw", u32::MAX)]);
        stock(&mut draft, &[("raw", u32::MAX)]);
        assert_eq!(
            pooled_genesis(&draft, &["raw".to_owned()]),
            Some(vec![8_589_934_590])
        );
        let choice = |id: &str, item: &str, count| CharacterCreationChoice {
            id: id.to_owned(),
            display_name: id.to_owned(),
            summary: "Choice".to_owned(),
            patch: CharacterPatch {
                inventory: BTreeMap::from([(item.to_owned(), count)]),
                ..CharacterPatch::default()
            },
        };
        draft.character_creation = Some(CharacterCreationDefinition {
            base: CharacterPatch {
                inventory: BTreeMap::from([("left".to_owned(), 1)]),
                ..CharacterPatch::default()
            },
            slots: vec![
                CharacterCreationSlot {
                    id: "a".to_owned(),
                    display_name: "A".to_owned(),
                    order: 0,
                    choices: vec![choice("a.left", "left", 2), choice("a.right", "right", 3)],
                },
                CharacterCreationSlot {
                    id: "b".to_owned(),
                    display_name: "B".to_owned(),
                    order: 1,
                    choices: vec![choice("b.left", "left", 4), choice("b.right", "right", 5)],
                },
            ],
        });
        // The deliberately overstocked pool includes both mutually exclusive
        // maxima. It dominates all four legal custom combinations itemwise.
        assert_eq!(
            pooled_genesis(&draft, &["left".to_owned(), "right".to_owned()]),
            Some(vec![7, 8])
        );
    }
}
