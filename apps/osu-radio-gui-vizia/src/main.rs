pub(crate) mod app;
pub(crate) mod assets;
mod gallery;
mod input;
mod launch;
pub(crate) mod views;

use std::process::ExitCode;

use tokio::runtime::Runtime;

fn main() -> ExitCode {
    // Choose the standalone gallery before constructing any runtime or application model.
    match launch::LaunchMode::parse(std::env::args_os().skip(1)) {
        Ok(launch::LaunchMode::ComponentGallery) => return finish(gallery::run()),
        Ok(launch::LaunchMode::Help) => {
            println!("osu-radio-gui-vizia [--component-gallery]");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
        Ok(launch::LaunchMode::Player) => {}
    }
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

    finish(result)
}

fn finish(result: Result<(), vizia::prelude::ApplicationError>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
