//! Conservative result-word bounds for canonically scheduled deferred events.
//!
//! If each template has exactly one scheduling occurrence, directly in the
//! same action without random effects, every successful execution schedules
//! those templates together. Atomic effects and permanent one-shot identities
//! preserve their relative due offsets, including preceding time advances.
//! Thus only deadlines separated by less than a transition's tick count can
//! resolve together. Different scheduling actions remain independently aligned.
//!
//! This proof applies to canonical histories. Structural state admission does
//! not authenticate the action that scheduled a template; the runtime's actual
//! observation-word check still rejects oversized bodies from forged ledgers.

use crate::{ContentDraft, DeferredEventDefinition, Effect};
use std::collections::BTreeMap;

#[derive(Default)]
struct ScheduleUse {
    count: usize,
    fixed_occurrence: Option<(usize, u64)>,
}

pub(super) fn maximum_result_words(draft: &ContentDraft, ticks: u64) -> usize {
    if ticks == 0 {
        return 0;
    }
    let templates: BTreeMap<_, _> = draft
        .deferred_events
        .iter()
        .map(|event| (event.id.as_str(), event))
        .collect();
    let independent_sum = || {
        draft
            .deferred_events
            .iter()
            .map(|event| event.result.split_whitespace().count())
            .fold(0, usize::saturating_add)
    };
    if templates.len() != draft.deferred_events.len() {
        return independent_sum();
    }
    let mut uses = BTreeMap::new();
    for (index, action) in draft.actions.iter().enumerate() {
        let fixed_duration = action
            .effects
            .iter()
            .try_fold(0u64, |total, effect| match effect {
                Effect::RandomChance { .. } => None,
                Effect::AdvanceTime { ticks } => total.checked_add(*ticks),
                _ => Some(total),
            });
        let mut prefix = Some(0u64);
        for effect in &action.effects {
            let occurrence = fixed_duration.and_then(|_| prefix.map(|time| (index, time)));
            collect_schedule_uses(effect, occurrence, &templates, &mut uses);
            if let Effect::AdvanceTime { ticks } = effect {
                prefix = prefix.and_then(|time| time.checked_add(*ticks));
            }
        }
    }
    // Event programs cannot schedule in validated content. Still count their
    // occurrences as ambiguous so the helper is conservative on draft inputs.
    for effect in draft
        .timed_events
        .iter()
        .flat_map(|event| &event.effects)
        .chain(
            draft
                .deferred_events
                .iter()
                .flat_map(|event| &event.effects),
        )
    {
        collect_schedule_uses(effect, None, &templates, &mut uses);
    }

    let mut independent = 0usize;
    let mut cohorts: BTreeMap<usize, Vec<(u64, usize)>> = BTreeMap::new();
    for event in &draft.deferred_events {
        let words = event.result.split_whitespace().count();
        match uses.get(event.id.as_str()) {
            Some(ScheduleUse {
                count: 1,
                fixed_occurrence: Some((action, offset)),
            }) => cohorts.entry(*action).or_default().push((*offset, words)),
            _ => independent = independent.saturating_add(words),
        }
    }
    for cohort in cohorts.values_mut() {
        cohort.sort_unstable();
        // Use a fresh sum per left endpoint. Saturating sums must not later
        // be subtracted: overflow could otherwise make the bound too small.
        let maximum = cohort
            .iter()
            .enumerate()
            .map(|(left, (start, _))| {
                cohort[left..]
                    .iter()
                    .take_while(|(due, _)| due - start < ticks)
                    .map(|(_, words)| *words)
                    .fold(0, usize::saturating_add)
            })
            .max()
            .unwrap_or_default();
        independent = independent.saturating_add(maximum);
    }
    independent
}

fn collect_schedule_uses<'a>(
    effect: &'a Effect,
    occurrence: Option<(usize, u64)>,
    templates: &BTreeMap<&str, &DeferredEventDefinition>,
    uses: &mut BTreeMap<&'a str, ScheduleUse>,
) {
    match effect {
        Effect::ScheduleEvent { event } => {
            let entry = uses.entry(event.as_str()).or_default();
            entry.count = entry.count.saturating_add(1);
            entry.fixed_occurrence = occurrence.and_then(|(action, prefix)| {
                templates.get(event.as_str()).and_then(|template| {
                    prefix
                        .checked_add(template.delay)
                        .map(|offset| (action, offset))
                })
            });
        }
        Effect::RandomChance {
            on_success,
            on_failure,
            ..
        } => {
            collect_schedule_uses(on_success, None, templates, uses);
            collect_schedule_uses(on_failure, None, templates, uses);
        }
        Effect::Noop
        | Effect::SetFlag { .. }
        | Effect::SetWorldFlag { .. }
        | Effect::SetLocationFlag { .. }
        | Effect::AdjustResource { .. }
        | Effect::MoveCharacter { .. }
        | Effect::MoveNpc { .. }
        | Effect::AdjustNpcRelationship { .. }
        | Effect::AddNpcMemory { .. }
        | Effect::TeachNpc { .. }
        | Effect::TransferNpcItemToCharacter { .. }
        | Effect::TransferStorageItemToCharacter { .. }
        | Effect::TransferCharacterItemToStorage { .. }
        | Effect::ApplyRecipe { .. }
        | Effect::AddCharacterDeed { .. }
        | Effect::AdvanceTime { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ActionDefinition, BuildManifest, Condition};

    fn schedule(event: &str) -> Effect {
        Effect::ScheduleEvent {
            event: event.to_owned(),
        }
    }

    fn action(id: &str, effects: Vec<Effect>) -> ActionDefinition {
        serde_json::from_value(serde_json::json!({
            "id": id, "label": "Schedule", "effects": effects,
            "meaningful": false, "movement": false
        }))
        .unwrap()
    }

    fn draft() -> ContentDraft {
        let manifest = BuildManifest::generated();
        let mut draft: ContentDraft = serde_json::from_value(serde_json::json!({
            "schema_version": manifest.schema_abi_version(),
            "rules_version": manifest.rules_abi_version(),
            "world_id": "budget"
        }))
        .unwrap();
        draft.deferred_events = [
            ("test.first", 2, "First ready."),
            ("test.second", 5, "Second batch ready."),
        ]
        .into_iter()
        .map(|(id, delay, result)| DeferredEventDefinition {
            id: id.to_owned(),
            delay,
            event_kind: "ready".to_owned(),
            label: "Ready".to_owned(),
            result: result.to_owned(),
            condition: Condition::Always,
            effects: vec![Effect::Noop],
        })
        .collect();
        draft.actions = vec![action(
            "test.schedule",
            vec![schedule("test.first"), schedule("test.second")],
        )];
        draft
    }

    #[test]
    fn cohort_windows_preserve_unequal_offsets_ties_and_multitick_crossings() {
        let mut draft = draft();
        assert_eq!(maximum_result_words(&draft, 0), 0);
        assert_eq!(maximum_result_words(&draft, 1), 3);
        assert_eq!(maximum_result_words(&draft, 3), 3);
        assert_eq!(maximum_result_words(&draft, 4), 5);
        draft.deferred_events[1].delay = 2;
        assert_eq!(maximum_result_words(&draft, 1), 5);
    }

    #[test]
    fn scheduling_prefix_time_changes_relative_deadlines() {
        let mut draft = draft();
        draft.actions[0].effects = vec![
            schedule("test.second"),
            Effect::AdvanceTime { ticks: 3 },
            schedule("test.first"),
        ];
        // Delays five and two coincide because the second schedule is later.
        assert_eq!(maximum_result_words(&draft, 1), 5);
        draft.actions[0]
            .effects
            .insert(0, Effect::AdvanceTime { ticks: 7 });
        assert_eq!(maximum_result_words(&draft, 1), 5);
        draft.actions[0].effects[2] = Effect::AdvanceTime { ticks: 2 };
        assert_eq!(maximum_result_words(&draft, 1), 3);
        assert_eq!(maximum_result_words(&draft, 2), 5);
    }

    #[test]
    fn separate_actions_and_unused_templates_remain_independently_aligned() {
        let mut draft = draft();
        draft.actions[0].effects.pop();
        assert_eq!(maximum_result_words(&draft, 1), 5, "unused is conservative");
        draft
            .actions
            .push(action("test.other", vec![schedule("test.second")]));
        assert_eq!(
            maximum_result_words(&draft, 1),
            5,
            "separate starts can align"
        );
    }

    #[test]
    fn duplicate_nested_and_random_schedules_use_independent_fallback() {
        let original = draft();
        let mut duplicate = original.clone();
        duplicate.actions[0].effects.push(schedule("test.first"));
        assert_eq!(maximum_result_words(&duplicate, 1), 5);
        let mut elsewhere = original.clone();
        elsewhere
            .actions
            .push(action("test.other", vec![schedule("test.first")]));
        assert_eq!(maximum_result_words(&elsewhere, 1), 5);
        for percent in [0, 50, 100] {
            let mut nested = original.clone();
            nested.actions[0].effects[0] = Effect::RandomChance {
                success_percent: percent,
                on_success: Box::new(schedule("test.first")),
                on_failure: Box::new(Effect::Noop),
            };
            assert_eq!(maximum_result_words(&nested, 1), 5);
            let mut unrelated_random = original.clone();
            unrelated_random.actions[0]
                .effects
                .push(Effect::RandomChance {
                    success_percent: percent,
                    on_success: Box::new(Effect::Noop),
                    on_failure: Box::new(Effect::Noop),
                });
            assert_eq!(maximum_result_words(&unrelated_random, 1), 5);
        }
        let mut event_schedule = original;
        event_schedule.deferred_events[1]
            .effects
            .push(schedule("test.first"));
        assert_eq!(maximum_result_words(&event_schedule, 1), 5);
    }

    #[test]
    fn overflowing_offsets_and_durations_cannot_establish_a_cohort() {
        let mut draft = draft();
        draft.actions[0]
            .effects
            .insert(0, Effect::AdvanceTime { ticks: u64::MAX });
        assert_eq!(
            maximum_result_words(&draft, 1),
            5,
            "prefix plus delay overflows"
        );
        draft.actions[0].effects.remove(0);
        draft.actions[0].effects.extend([
            Effect::AdvanceTime { ticks: u64::MAX },
            Effect::AdvanceTime { ticks: 1 },
        ]);
        assert_eq!(
            maximum_result_words(&draft, 1),
            5,
            "whole action duration overflows"
        );
    }
}
