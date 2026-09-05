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
        total = costs
            .into_iter()
            .take(slots)
            .fold(total, usize::saturating_add);
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
        serde_json::from_str(r#"{"schema_version":"forge-schema-v9","rules_version":"forge-rules-v7","world_id":"fixture"}"#).unwrap()
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
}
