mod cli;

fn main() {
    let code = match cli::execute() {
        Err(err) => {
            use std::io::Write;
            writeln!(std::io::stderr(), "teetty: {}", err).ok();
            1
        }
        Ok(code) => code,
    };
    std::process::exit(code);
}
