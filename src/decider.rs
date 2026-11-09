//! Optional contradiction decision interface; no model is enabled by default.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// An optional decision and the evidence cited to support it.
pub struct Decision {
    pub verdict: String,
    pub confidence: f64,
    pub citations: Vec<String>,
}
/// Interface for optional decisions about two instruction fragments.
pub trait Decider {
    /// Return a decision, or `None` when no decision can be made.
    fn decide(&self, first: &str, second: &str, context: &str) -> Option<Decision>;
}
#[derive(Debug, Default, Clone, Copy)]
/// A decider that never returns a decision.
pub struct NoDecider;
impl Decider for NoDecider {
    fn decide(&self, _first: &str, _second: &str, _context: &str) -> Option<Decision> {
        None
    }
}
