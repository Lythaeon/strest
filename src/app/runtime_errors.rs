pub(crate) fn print_runtime_errors(errors: &[String]) {
    eprintln!("Runtime errors:");
    for error in errors {
        eprintln!("- {}", error);
    }
}
