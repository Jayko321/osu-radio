pub(crate) mod app;
pub(crate) mod assets;
pub(crate) mod views;

use std::process::ExitCode;

use tokio::runtime::Runtime;

fn main() -> ExitCode {
    let runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("The async runtime could not be started: {error}");
            return ExitCode::FAILURE;
        }
    };

    // The runtime has to outlive the window: the embedded server is killed when the model holding
    // it drops, which happens inside `run`, and reaping the child needs a live runtime.
    let result = app::run(runtime.handle().clone());

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
