//! gear-memory — Local agentic context layer: code map, repo memory, and document/search substrate for coding agents.
//!
//! This crate is intentionally a minimal skeleton. The first implementation
//! increments must keep the upstream boundary explicit and preserve the
//! sovereign constraints documented in `docs/adr/0001-scope-and-upstream-policy.md`.

/// Static project metadata used by the CLI and smoke tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectCard {
    pub name: &'static str,
    pub role: &'static str,
    pub upstream: &'static str,
    pub relationship: &'static str,
}

/// The repository's initial scope card.
pub const PROJECT: ProjectCard = ProjectCard {
    name: "gear-memory",
    role: "local agentic context",
    upstream: "basemind",
    relationship: "Dev/operator tool only for Presto-Matic and cos-matic; never a product runtime dependency.",
};

/// Human-readable summary for CLI smoke runs.
pub fn summary() -> String {
    format!(
        "{} — {} (upstream: {})",
        PROJECT.name, PROJECT.role, PROJECT.upstream
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_card_names_the_repo_and_upstream() {
        assert_eq!(PROJECT.name, "gear-memory");
        assert_eq!(PROJECT.upstream, "basemind");
        assert!(summary().contains(PROJECT.role));
    }
}
