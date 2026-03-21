use std::path::PathBuf;

use derive_setters::Setters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Represents a reusable skill with a name, file path, and prompt content
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters, JsonSchema)]
#[setters(strip_option, into)]
pub struct Skill {
    /// Name of the skill
    pub name: String,

    /// File path to the skill markdown file
    pub path: Option<PathBuf>,

    /// Content/prompt loaded from the markdown file
    pub command: String,

    /// Description of the skill
    pub description: String,

    /// List of resource files in the skill directory
    pub resources: Vec<PathBuf>,
}

impl Skill {
    /// Creates a new Skill with required fields
    ///
    /// # Arguments
    ///
    /// * `name` - The name identifier for the skill
    /// * `prompt` - The skill prompt content
    /// * `description` - A brief description of the skill
    pub fn new(
        name: impl Into<String>,
        prompt: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            path: None,
            command: prompt.into(),
            description: description.into(),
            resources: Vec::new(),
        }
    }
}

/// Simplified skill information used when requesting skill selection.
///
/// Contains only the name and description fields required to send a skill
/// selection request to the remote ranking service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInfo {
    /// Name of the skill
    pub name: String,
    /// Description of the skill
    pub description: String,
}

impl SkillInfo {
    /// Creates a new skill info entry.
    ///
    /// # Arguments
    /// * `name` - The skill identifier
    /// * `description` - A brief description of what the skill does
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self { name: name.into(), description: description.into() }
    }
}

/// A skill selected based on relevance to a user prompt.
///
/// Holds the name and relevance score returned after ranking available skills
/// against a user query. Used to inject skill hints as droppable context
/// messages before an LLM turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct SelectedSkill {
    /// Name of the selected skill
    pub name: String,
    /// Relevance score (0.0–1.0) of the skill against the user prompt
    pub relevance: f32,
    /// 1-based rank of this skill in the selection results
    pub rank: u64,
}

impl SelectedSkill {
    /// Creates a new selected skill entry.
    ///
    /// # Arguments
    /// * `name` - The skill identifier
    /// * `relevance` - How relevant the skill is (0.0–1.0)
    /// * `rank` - Position in the ranked list (1-based)
    pub fn new(name: impl Into<String>, relevance: f32, rank: u64) -> Self {
        Self { name: name.into(), relevance, rank }
    }
}

impl From<&SelectedSkill> for forge_template::Element {
    fn from(skill: &SelectedSkill) -> Self {
        forge_template::Element::new("skill").attr("name", &skill.name)
    }
}

/// Request parameters for skill selection.
///
/// Bundles the list of available skills and the user's prompt into a single
/// request to send to the remote ranking service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSelectionParams {
    /// List of available skills to select from
    pub skills: Vec<SkillInfo>,
    /// User's prompt to match skills against
    pub user_prompt: String,
}

impl SkillSelectionParams {
    /// Creates new skill selection parameters.
    ///
    /// # Arguments
    /// * `skills` - The candidate skills to rank
    /// * `user_prompt` - The user query used to score relevance
    pub fn new(skills: Vec<SkillInfo>, user_prompt: impl Into<String>) -> Self {
        Self { skills, user_prompt: user_prompt.into() }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_skill_creation() {
        // Fixture
        let fixture = Skill::new(
            "code_review",
            "Review this code",
            "A skill for reviewing code quality",
        )
        .path("/skills/code_review.md");

        // Act
        let actual = (
            fixture.name.clone(),
            fixture.path.clone(),
            fixture.command.clone(),
            fixture.description.clone(),
        );

        // Assert
        let expected = (
            "code_review".to_string(),
            Some("/skills/code_review.md".into()),
            "Review this code".to_string(),
            "A skill for reviewing code quality".to_string(),
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_skill_with_setters() {
        // Fixture
        let fixture = Skill::new("test", "prompt", "desc")
            .path("/path")
            .name("updated_name")
            .path("/updated/path")
            .command("updated prompt")
            .description("updated description");

        // Act
        let actual = fixture;

        // Assert
        let expected = Skill::new("updated_name", "updated prompt", "updated description")
            .path("/updated/path");
        assert_eq!(actual, expected);
    }
}
