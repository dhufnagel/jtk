use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--debug-tree") {
        return debug_tree();
    }
    let code = jtk::cli::run(
        &args,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    ExitCode::from(code as u8)
}

/// Prints the tree-sitter syntax tree of stdin (for debugging the parser adapter).
fn debug_tree() -> ExitCode {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    match jtk::parse::parse(&input) {
        Some(tree) => {
            println!("{}", tree.root_node().to_sexp());
            ExitCode::SUCCESS
        }
        None => ExitCode::FAILURE,
    }
}
