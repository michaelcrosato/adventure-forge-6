//! Authoring boundary for Adventure Forge content.
//!
//! Parsing is intentionally kept here while semantic validation and
//! construction remain in `forge-kernel`. This prevents a content producer
//! from bypassing the kernel's trust boundary.

pub use forge_kernel::{
    ActionDefinition, ActionPage, ActionView, CharacterPreset, Condition, ContentContract,
    ContentDraft, Effect, LocationDefinition, NpcDefinition, Observation, ParameterDomain,
    ParameterSpec, StringRef, TextVariant,
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
            locations: Vec::new(),
            npcs: Vec::new(),
            actions: Vec::new(),
        };
        let error = compile_production(source).unwrap_err();
        assert_eq!(
            error.issues,
            vec!["production compilation requires contract=production"]
        );
    }
}
