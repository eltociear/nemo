//! Contains the trait that defines what constitutes a rule execution strategy.

use crate::logical::{model::Rule, program_analysis::analysis::RuleAnalysis};

/// Trait that defines a strategy for rule execution,
/// namely the order in which the rules are applied in.
pub trait RuleSelectionStrategy: std::fmt::Debug {
    /// Create a new [`RuleSelectionStrategy`] object.
    fn new(rules: Vec<&Rule>, rule_analyses: Vec<&RuleAnalysis>) -> Self;

    /// Return the index of the next rule that should be executed.
    /// Returns `None` if there are no more rules to be applied
    /// and the execution should therefore stop.
    fn next_rule(&mut self, new_derivations: Option<bool>) -> Option<usize>;
}
