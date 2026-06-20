use caternary::*;

fn rspan() -> RefineSpan {
    RefineSpan { start: 0, end: 0 }
}
fn span() -> Span {
    Span { start: 0, end: 0 }
}

fn main() {
    let toks = parse("x 0 > [ x sqrt ] [ 0 ] if").unwrap();
    let mut stack = ShadowStack::new();
    let mut solver = SmtLibSolver::new();
    let mut obligations = Vec::new();
    let resolve = |w: &str| -> VerifyWord {
        match w {
            "sqrt" => VerifyWord::Call {
                binders: vec![Binder {
                    name: "n".into(),
                    ty: "Num".into(),
                    span: rspan(),
                }],
                demand: Some(Pred::Bin(
                    BinOp::Ge,
                    Box::new(Pred::Var("n".into())),
                    Box::new(Pred::Num("0".into())),
                )),
                out_binders: vec![Binder {
                    name: "r".into(),
                    ty: "Num".into(),
                    span: rspan(),
                }],
                guarantee: None,
                arrow: WordTy::new(
                    StackTy::new(vec![Ty::num(span())], 0, span()),
                    StackTy::new(vec![Ty::num(span())], 0, span()),
                ),
            },
            other => {
                if let Some(c) = core_shadow_word(other) {
                    VerifyWord::Core(c)
                } else if let Some(op) = interpreted_op(other) {
                    VerifyWord::Core(op)
                } else if is_numeric_literal(other) {
                    VerifyWord::Core(ShadowWord::Num(other.to_string()))
                } else {
                    VerifyWord::Core(ShadowWord::Var(other.to_string()))
                }
            }
        }
    };
    verify(&toks, &mut stack, &mut solver, &resolve, &mut obligations).unwrap();
    println!("=== SMT-LIB script ===\n{}", solver.script());
    println!("=== obligations ===");
    for o in &obligations {
        println!("{:?}", o);
    }
}
