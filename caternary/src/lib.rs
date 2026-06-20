//! Caternary: a concatenative language that enables pattern-based optimization.

#![deny(missing_docs)]

mod builtins;
mod check;
mod combinators;
mod evaluator;
mod optimizer;
mod parser;
mod refinement;
mod shadow;
mod solver;
mod types;

pub use builtins::register_stack_builtins;
pub use check::TypeError;
pub use check::check;
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
pub use refinement::BinOp;
pub use refinement::Binder;
pub use refinement::Pred;
pub use refinement::RefineParseError;
pub use refinement::RefineSpan;
pub use refinement::RefinementSide;
pub use refinement::RefinementSig;
pub use refinement::UnOp;
pub use refinement::parse_predicate;
pub use refinement::parse_signature;
pub use shadow::NamedBinding;
pub use shadow::ShadowError;
pub use shadow::ShadowStack;
pub use shadow::ShadowWord;
pub use shadow::Slot;
pub use shadow::bind_positional;
pub use shadow::core_shadow_word;
pub use shadow::interpreted_op;
pub use solver::CounterModel;
pub use solver::Model;
pub use solver::Obligation;
pub use solver::SUBSUMPTION_FAIL_CLOSED_MSG;
pub use solver::SmtLibSolver;
pub use solver::Solver;
pub use solver::SubsumptionDirection;
pub use solver::SubsumptionOutcome;
pub use solver::SubsumptionResult;
pub use solver::SubsumptionVc;
pub use solver::Verdict;
pub use solver::VerifyResolve;
pub use solver::VerifyWord;
pub use solver::check_refinements;
pub use solver::check_sat;
pub use solver::check_sat_model;
pub use solver::check_subsumption;
pub use solver::discharge;
pub use solver::discharge_with_model;
pub use solver::negate;
pub use solver::refinement_verify_word;
pub use solver::relay_quote_post;
pub use solver::render_smtlib;
pub use solver::substitute;
pub use solver::verify;
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
pub use types::core_scheme;
pub use types::is_bool_literal;
pub use types::is_numeric_literal;
pub use types::respan_word;

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
