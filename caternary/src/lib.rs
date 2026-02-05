//! Caternary: a concatenative language that enables pattern-based optimization.

#![deny(missing_docs)]

mod builtins;
mod evaluator;
mod optimizer;
mod parser;

pub use builtins::register_stack_builtins;
pub use evaluator::EvalError;
pub use evaluator::Evaluator;
pub use evaluator::Operator;
pub use optimizer::Optimizer;
pub use optimizer::Rule;
pub use optimizer::RuleError;
pub use parser::ParseError;
pub use parser::Token;
pub use parser::parse;
