//! Authoring boundary for Adventure Forge content.
//!
//! Parsing is intentionally kept here while semantic validation and
//! construction remain in `forge-kernel`. This prevents a content producer
//! from bypassing the kernel's trust boundary.

pub use forge_kernel::{
    ActionDefinition, Condition, ContentDraft, Effect, LocationDefinition, NpcDefinition,
    ParameterDomain, ParameterSpec, StringRef,
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

pub fn compile(source: ContentSource) -> Result<CompiledContent, ContentError> {
    CompiledContent::try_compile(source).map_err(validation_error)
}
