//! Caternary: a concatenative language that enables pattern-based optimization.

#![deny(missing_docs)]

mod builtins;
mod check;
mod combinators;
mod evaluator;
mod optimizer;
mod parser;
mod types;

pub use builtins::register_stack_builtins;
pub use check::TypeError;
pub use check::type_check;
pub use check::type_check_entry;
pub use combinators::Quotable;
pub use combinators::register_combinators;
pub use combinators::register_conditionals;
pub use combinators::register_sequence_combinators;
pub use evaluator::CODE_OPERATOR_ERROR;
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
pub use types::BOOL;
pub use types::FrameStack;
pub use types::InferCtx;
pub use types::MAIN;
pub use types::NUM;
pub use types::RowVar;
pub use types::Scheme;
pub use types::StackTy;
pub use types::Subst;
pub use types::Ty;
pub use types::TyKind;
pub use types::TyVar;
pub use types::TypingFrame;
pub use types::UnifyError;
pub use types::WordTy;
pub use types::is_numeric_literal;

/// Register all builtins and combinators on an evaluator.
pub fn register_all_builtins<T>(evaluator: &mut Evaluator<T>)
where
    T: Quotable,
{
    register_stack_builtins(evaluator);
    register_combinators(evaluator);
    register_conditionals(evaluator);
    register_sequence_combinators(evaluator);
}
