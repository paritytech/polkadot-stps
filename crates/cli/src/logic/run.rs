use crate::prelude::*;

async fn try_run(cli_args: CliArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::try_from(cli_args)?;
    info!("{} is running with args: {:#?}", BINARY_NAME, config);
    Ok(())
}

pub async fn run(cli_args: CliArgs) {
    match try_run(cli_args).await {
        Ok(_) => info!("{} ran successfully", BINARY_NAME),
        Err(e) => error!("Error running {}: {}", BINARY_NAME, e),
    }
}
