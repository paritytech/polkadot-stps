use crate::prelude::*;

fn try_run(cli_args: CliArgs) -> Result<(), Box<dyn std::error::Error>> {
    let _config = Config;
    info!("{} is running...", BINARY_NAME);
    Ok(())
}

pub fn run(cli_args: CliArgs) {
    match try_run(cli_args) {
        Ok(_) => info!("{} ran successfully", BINARY_NAME),
        Err(e) => error!("Error running {}: {}", BINARY_NAME, e),
    }
}
