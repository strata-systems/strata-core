//! CLI session context.

/// Branch and space selected for a command.
#[derive(Clone, Debug, Default)]
pub(crate) struct Scope {
    pub(crate) branch: Option<String>,
    pub(crate) space: Option<String>,
}

/// Mutable shell context shared by REPL and pipe execution.
#[cfg(feature = "native")]
#[derive(Clone, Debug, Default)]
pub(crate) struct CommandContext {
    branch: Option<String>,
    space: Option<String>,
    /// The provider-key environment variables the CLI filled from the config
    /// file when the process started. The bridge runs once, before any
    /// command, so this is the only record of which keys the file supplied —
    /// what lets `inference status` name the file, one-shot or mid-session.
    #[cfg(feature = "inference")]
    config_backed_keys: Vec<&'static str>,
}

#[cfg(feature = "native")]
impl CommandContext {
    pub(crate) fn new(branch: Option<String>, space: Option<String>) -> Self {
        Self {
            branch,
            space,
            #[cfg(feature = "inference")]
            config_backed_keys: Vec::new(),
        }
    }

    #[cfg(feature = "inference")]
    pub(crate) fn set_config_backed_keys(&mut self, keys: Vec<&'static str>) {
        self.config_backed_keys = keys;
    }

    #[cfg(feature = "inference")]
    pub(crate) fn config_backed_keys(&self) -> &[&'static str] {
        &self.config_backed_keys
    }

    pub(crate) fn scope_with_overrides(
        &self,
        branch: Option<String>,
        space: Option<String>,
    ) -> Scope {
        Scope {
            branch: branch.or_else(|| self.branch.clone()),
            space: space.or_else(|| self.space.clone()),
        }
    }

    pub(crate) fn set_branch(&mut self, branch: String) {
        self.branch = Some(branch);
    }

    pub(crate) fn set_space(&mut self, space: Option<String>) {
        self.space = space;
    }

    pub(crate) fn branch_or_default<'a>(&'a self, executor_default: &'a str) -> &'a str {
        self.branch.as_deref().unwrap_or(executor_default)
    }

    pub(crate) fn space_or_default(&self) -> &str {
        self.space
            .as_deref()
            .unwrap_or(strata_executor::DEFAULT_SPACE)
    }

    pub(crate) fn prompt(&self, executor_default: &str) -> String {
        let branch = self.branch_or_default(executor_default);
        format!("strata:{branch}/{}> ", self.space_or_default())
    }
}
