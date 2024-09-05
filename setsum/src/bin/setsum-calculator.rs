//! A calculator for adding, subtracting, and manipulating setsums.

use std::io::BufRead;

use setsum::Setsum;

fn print(stack: &[Setsum]) {
    if stack.len() > 1 {
        println!(
            "{} (+{} more value(s) on the stack)",
            stack[stack.len() - 1].hexdigest(),
            stack.len() - 1
        );
    } else if !stack.is_empty() {
        println!("{}", stack[0].hexdigest());
    } else {
        println!("empty stack");
    }
}

fn main() {
    let mut stack = vec![];
    for line in std::io::stdin().lock().lines() {
        let line = line.expect("no I/O errors should be encountered reading stdin");
        if let Some(setsum) = Setsum::from_hexdigest(&line) {
            stack.push(setsum);
        } else if line.trim() == "pop" {
            stack.pop();
        } else if line.trim() == "+" {
            if stack.len() >= 2 {
                // SAFETY(rescrv):  stack.len() >= 2.
                let rhs = stack.pop().unwrap();
                let lhs = stack.pop().unwrap();
                stack.push(lhs + rhs);
                print(&stack);
            } else {
                eprintln!("need at least two items on the stack to add");
            }
        } else if line.trim() == "-" {
            if stack.len() >= 2 {
                // SAFETY(rescrv):  stack.len() >= 2.
                let rhs = stack.pop().unwrap();
                let lhs = stack.pop().unwrap();
                stack.push(lhs - rhs);
                print(&stack);
            } else {
                eprintln!("need at least two items on the stack to subtract");
            }
        } else if line.trim() == "!" {
            if !stack.is_empty() {
                // SAFETY(rescrv):  stack.len() >= 2.
                let lhs = stack.pop().unwrap();
                stack.push(Setsum::default() - lhs);
                print(&stack);
            } else {
                eprintln!("need at least one item on the stack to take inverse");
            }
        } else {
            eprintln!("don't know how to parse {line} as setsum or operation");
        }
    }
}
