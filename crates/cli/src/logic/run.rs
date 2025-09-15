use crate::prelude::*;

async fn try_run(cli_args: CliArgs) -> Result<(), CliError> {
    let parameters = Parameters::try_from(cli_args)?;
    let mut spammer = Spammer::bootstrap(parameters).await?;
    spammer.run().await.map_err(CliError::CoreError)
}

pub async fn run(cli_args: CliArgs) {
    match try_run(cli_args).await {
        Ok(_) => info!("{} ran successfully", BINARY_NAME),
        Err(e) => error!("Error running {}: {}", BINARY_NAME, e),
    }
}
