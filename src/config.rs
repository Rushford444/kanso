//! Configuration and validation.
use crate::KansoError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
/// Analysis limits, enabled rules and discovery exclusions.
pub struct Config {
    /// Maximum bytes read from an instruction file or linked resource.
    pub max_file_bytes: usize,
    pub token_warn: usize,
    pub token_error: usize,
    pub total_token_warn: usize,
    pub total_token_error: usize,
    pub similarity_threshold: f64,
    pub disabled_rules: Vec<String>,
    pub ignore: Vec<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            max_file_bytes: 2 * 1024 * 1024,
            token_warn: 2000,
            token_error: 6000,
            total_token_warn: 10000,
            total_token_error: 20000,
            similarity_threshold: 0.85,
            disabled_rules: Vec::new(),
            ignore: Vec::new(),
        }
    }
}
impl Config {
    /// Load and validate a TOML config file.
    ///
    /// # Errors
    /// Returns an error if reading, parsing or validation fails.
    pub fn load(path: &Path) -> Result<Self, KansoError> {
        let text = crate::discovery::read_regular(path, Self::default().max_file_bytes)?;
        let config: Self = toml::from_str(&text).map_err(|e| KansoError::Config(format!("{e}")))?;
        config.validate()?;
        Ok(config)
    }
    /// Validate bounds and configured rule IDs.
    ///
    /// # Errors
    /// Returns an error for inconsistent limits or unrecognized rule IDs.
    pub fn validate(&self) -> Result<(), KansoError> {
        if self.max_file_bytes == 0 {
            return Err(KansoError::Config(
                "max_file_bytes must be greater than zero".into(),
            ));
        }
        if self.token_warn > self.token_error || self.total_token_warn > self.total_token_error {
            return Err(KansoError::Config(
                "warning threshold must not exceed error threshold".into(),
            ));
        }
        if !self.similarity_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.similarity_threshold)
        {
            return Err(KansoError::Config(
                "similarity threshold must be between 0 and 1".into(),
            ));
        }
        for rule in &self.disabled_rules {
            if !(1..=9).any(|n| rule == &format!("KS{n:03}")) {
                return Err(KansoError::Config(format!("unknown disabled rule: {rule}")));
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_thresholds_are_rejected() {
        let mut c = Config {
            token_warn: 3,
            token_error: 2,
            ..Config::default()
        };
        assert!(c.validate().is_err());
        c.token_warn = 1;
        c.similarity_threshold = f64::NAN;
        assert!(c.validate().is_err());
    }
}
