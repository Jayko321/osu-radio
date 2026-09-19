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

    // `run` retains the controller through window teardown and awaits child cleanup before
    // returning. Blocking image decoders also finish before this runtime is dropped.
    let result = app::run(runtime.handle());

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
