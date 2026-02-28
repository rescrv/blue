//! Caternary: a concatenative language that enables pattern-based optimization.

#![deny(missing_docs)]

mod evaluator;
mod optimizer;
mod parser;

pub use evaluator::EvalError;
pub use evaluator::Evaluator;
pub use evaluator::Operator;
pub use optimizer::Optimizer;
pub use optimizer::Rule;
pub use optimizer::RuleError;
pub use parser::ParseError;
pub use parser::Span;
pub use parser::SpannedToken;
pub use parser::SpannedTokenKind;
pub use parser::Token;
pub use parser::parse;
pub use parser::parse_with_spans;
