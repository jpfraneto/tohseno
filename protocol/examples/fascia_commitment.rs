use std::path::PathBuf;
use std::process::ExitCode;
use tohseno_protocol::fascia_tree::hash_fascia_tree;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os();
    let program = arguments
        .next()
        .and_then(|value| PathBuf::from(value).file_name().map(|name| name.to_owned()))
        .unwrap_or_else(|| "fascia_commitment".into());
    let Some(root) = arguments.next() else {
        eprintln!("usage: {} <fascia-root>", program.to_string_lossy());
        return ExitCode::from(2);
    };
    if arguments.next().is_some() {
        eprintln!("usage: {} <fascia-root>", program.to_string_lossy());
        return ExitCode::from(2);
    }

    match hash_fascia_tree(&PathBuf::from(root)) {
        Ok(commitment) => {
            println!("{}", commitment.digest);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("fascia commitment failed: {error}");
            ExitCode::FAILURE
        }
    }
}
