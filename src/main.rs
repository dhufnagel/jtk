use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--debug-tree") {
        return debug_tree();
    }
    // The language server reads and writes stdin and stdout on its own threads, so they must not be locked here.
    if args == ["lsp"] {
        return ExitCode::from(jtk::lsp::run(&mut std::io::stderr()) as u8);
    }
    let code = jtk::cli::run(
        &args,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    ExitCode::from(code as u8)
}

/// Prints the syntax tree of stdin (for debugging the parser).
fn debug_tree() -> ExitCode {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    match jtk::parser::parse(&input) {
        Ok(tree) => {
            println!("{tree:#?}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("syntax error at byte {}", error.offset);
            ExitCode::FAILURE
        }
    }
}
