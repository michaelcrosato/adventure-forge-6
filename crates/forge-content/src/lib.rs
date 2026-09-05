//! Authoring boundary for Adventure Forge content.
//!
//! Parsing is intentionally kept here while semantic validation and
//! construction remain in `forge-kernel`. This prevents a content producer
//! from bypassing the kernel's trust boundary.

pub use forge_kernel::{
    ActionDefinition, ActionPage, ActionTimeCost, ActionView, CharacterCreationChoice,
    CharacterCreationDefinition, CharacterCreationSlot, CharacterPatch, CharacterPreset,
    CharacterSelection, Condition, ContentContract, ContentDraft, DeferredEventDefinition, Effect,
    LocationDefinition, NpcDefinition, Observation, ParameterDomain, ParameterSpec,
    RecipeDefinition, StorageDefinition, StringRef, TextVariant, TimedEventDefinition,
    TimedEventView,
};
pub type ContentSource = ContentDraft;
pub type LocationSource = LocationDefinition;

use forge_kernel::{CompiledContent, ContentValidationError};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentError {
    pub issues: Vec<String>,
}

impl Display for ContentError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (index, issue) in self.issues.iter().enumerate() {
            if index != 0 {
                f.write_str("; ")?;
            }
            f.write_str(issue)?;
        }
        Ok(())
    }
}

impl std::error::Error for ContentError {}

fn parse_error(message: impl Into<String>) -> ContentError {
    ContentError {
        issues: vec![message.into()],
    }
}

fn validation_error(error: ContentValidationError) -> ContentError {
    ContentError {
        issues: error.issues,
    }
}

pub fn parse(input: &str) -> Result<ContentSource, ContentError> {
    forge_kernel::validate_unique_json_keys(input)
        .map_err(|error| parse_error(format!("invalid content source: {error}")))?;
    serde_json::from_str(input)
        .map_err(|error| parse_error(format!("invalid content source: {error}")))
}

pub fn parse_and_compile(input: &str) -> Result<CompiledContent, ContentError> {
    compile(parse(input)?)
}

/// Parse a shippable content pack while requiring the production contract at
/// this trusted application boundary. An untrusted document cannot opt into
/// weaker fixture validation by omitting or changing its `contract` field.
pub fn parse_and_compile_production(input: &str) -> Result<CompiledContent, ContentError> {
    compile_production(parse(input)?)
}

pub fn compile(source: ContentSource) -> Result<CompiledContent, ContentError> {
    CompiledContent::try_compile(source).map_err(validation_error)
}

pub fn compile_production(source: ContentSource) -> Result<CompiledContent, ContentError> {
    if source.contract != ContentContract::Production {
        return Err(parse_error(
            "production compilation requires contract=production",
        ));
    }
    compile(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_compiler_rejects_fixture_opt_out_before_semantic_validation() {
        let source = ContentSource {
            schema_version: String::new(),
            rules_version: String::new(),
            world_id: String::new(),
            contract: ContentContract::Fixture,
            start_location: String::new(),
            character_presets: Vec::new(),
            character_creation: None,
            supply_labels: Default::default(),
            locations: Vec::new(),
            npcs: Vec::new(),
            storages: Vec::new(),
            timed_events: Vec::new(),
            deferred_events: Vec::new(),
            recipes: Vec::new(),
            actions: Vec::new(),
        };
        let error = compile_production(source).unwrap_err();
        assert_eq!(
            error.issues,
            vec!["production compilation requires contract=production"]
        );
    }

    #[test]
    fn parser_rejects_duplicate_keys_before_typed_maps_collapse_them() {
        let duplicate_top_level = r#"{
            "schema_version":"forge-schema-v10",
            "schema_version":"shadow"
        }"#;
        assert!(parse(duplicate_top_level).is_err());

        let duplicate_nested_map = r#"{
            "schema_version":"forge-schema-v10",
            "rules_version":"forge-rules-v8",
            "world_id":"world",
            "character_creation":{
                "base":{"resources":{"coin":1,"coin":2}},
                "slots":[]
            }
        }"#;
        assert!(parse(duplicate_nested_map).is_err());
    }

    #[test]
    fn recipe_parser_rejects_duplicate_item_keys_and_unknown_recipe_fields() {
        for recipes in [
            r#"[{"id":"test.press","inputs":{"test.clay":1,"test.clay":2},"outputs":{"test.repair":1}}]"#,
            r#"[{"id":"test.press","inputs":{"test.clay":1},"outputs":{"test.repair":1,"test.repair":2}}]"#,
            r#"[{"id":"test.press","inputs":{"test.clay":1},"outputs":{},"hidden_effect":"free_goods"}]"#,
            r#"[{"id":"test.press","inputs":{"test.clay":1}}]"#,
            r#"[{"id":"test.press","inputs":{"test.clay":-1},"outputs":{}}]"#,
            r#"[{"id":"test.press","inputs":{"test.clay":1.5},"outputs":{}}]"#,
            r#"[{"id":"test.press","inputs":{"test.clay":4294967296},"outputs":{}}]"#,
        ] {
            let input = format!(
                r#"{{"schema_version":"forge-schema-v10","rules_version":"forge-rules-v8","world_id":"test.world","recipes":{recipes}}}"#
            );
            assert!(
                parse(&input).is_err(),
                "malformed recipe must fail parsing: {recipes}"
            );
        }
        let source = parse(r#"{"schema_version":"forge-schema-v10","rules_version":"forge-rules-v8","world_id":"test.world"}"#).unwrap();
        assert!(source.recipes.is_empty());
        assert!(source.deferred_events.is_empty());
    }

    #[test]
    fn deferred_parser_rejects_ambiguous_fields_and_noninteger_delay() {
        let valid_template = r#"{"schema_version":"forge-schema-v10","rules_version":"forge-rules-v8","world_id":"test.world","deferred_events":[{"id":"test.ready","delay":2,"event_kind":"batch","label":"Ready","result":"Ready."}]}"#;
        let parsed = parse(valid_template).unwrap();
        assert_eq!(parsed.deferred_events[0].condition, Condition::Always);
        assert!(parsed.deferred_events[0].effects.is_empty());
        for event in [
            r#"{"id":"test.ready","delay":2,"delay":3,"event_kind":"batch","label":"Ready","result":"Ready."}"#,
            r#"{"id":"test.ready","delay":2,"due_time":3,"event_kind":"batch","label":"Ready","result":"Ready."}"#,
            r#"{"id":"test.ready","delay":-1,"event_kind":"batch","label":"Ready","result":"Ready."}"#,
            r#"{"id":"test.ready","delay":1.5,"event_kind":"batch","label":"Ready","result":"Ready."}"#,
            r#"{"id":"test.ready","delay":18446744073709551616,"event_kind":"batch","label":"Ready","result":"Ready."}"#,
            r#"{"id":"test.ready","event_kind":"batch","label":"Ready","result":"Ready."}"#,
        ] {
            let source = format!(
                r#"{{"schema_version":"forge-schema-v10","rules_version":"forge-rules-v8","world_id":"test.world","deferred_events":[{event}]}}"#
            );
            assert!(
                parse(&source).is_err(),
                "malformed deferred template must fail: {event}"
            );
        }
        let invalid_effect = r#"{"schema_version":"forge-schema-v10","rules_version":"forge-rules-v8","world_id":"test.world","actions":[{"id":"test.schedule","label":"Schedule","meaningful":true,"movement":false,"effects":[{"kind":"schedule_event","event":"test.ready","delay":2}]}]}"#;
        let valid_effect = invalid_effect.replace(",\"delay\":2", "");
        assert_eq!(
            parse(&valid_effect).unwrap().actions[0].effects,
            vec![Effect::ScheduleEvent {
                event: "test.ready".to_owned()
            }]
        );
        assert!(
            parse(invalid_effect).is_err(),
            "delay belongs only to the template"
        );
    }

    #[test]
    fn storage_parser_rejects_ambiguous_stock_and_untyped_transfer_fields() {
        for storage in [
            r#"{"id":"test.cage","name":"Cage","location":"test.room","inventory":{"test.filter":1,"test.filter":2}}"#,
            r#"{"id":"test.cage","name":"Cage","location":"test.room","capacity":10}"#,
            r#"{"id":"test.cage","name":"Cage","location":"test.room","inventory":{"test.filter":-1}}"#,
            r#"{"id":"test.cage","name":"Cage","location":"test.room","inventory":{"test.filter":1.5}}"#,
            r#"{"id":"test.cage","name":"Cage","location":"test.room","inventory":{"test.filter":4294967296}}"#,
            r#"{"id":"test.cage","name":"Cage"}"#,
        ] {
            let source = format!(
                r#"{{"schema_version":"forge-schema-v10","rules_version":"forge-rules-v8","world_id":"test.world","storages":[{storage}]}}"#
            );
            assert!(
                parse(&source).is_err(),
                "malformed storage must fail parsing: {storage}"
            );
        }
        for kind in [
            "transfer_storage_item_to_character",
            "transfer_character_item_to_storage",
        ] {
            for fields in [
                r#""storage":{"kind":"parameter","value":"stock"},"item":"test.filter","count":1"#,
                r#""storage":"test.cage","item":"test.filter","count":1,"npc":"test.custodian""#,
                r#""storage":"test.cage","item":"test.filter","count":1,"count":2"#,
                r#""storage":"test.cage","item":"test.filter","count":-1"#,
            ] {
                let source = format!(
                    r#"{{"schema_version":"forge-schema-v10","rules_version":"forge-rules-v8","world_id":"test.world","actions":[{{"id":"test.take","label":"Take","meaningful":true,"movement":false,"effects":[{{"kind":"{kind}",{fields}}}]}}]}}"#
                );
                assert!(
                    parse(&source).is_err(),
                    "storage transfers have fixed typed IDs and integer counts"
                );
            }
        }
        let empty = parse(r#"{"schema_version":"forge-schema-v10","rules_version":"forge-rules-v8","world_id":"test.world"}"#).unwrap();
        assert!(empty.storages.is_empty());
        let storage: StorageDefinition =
            serde_json::from_str(r#"{"id":"test.cage","name":"Cage","location":"test.room"}"#)
                .unwrap();
        assert!(storage.inventory.is_empty());
    }
}
