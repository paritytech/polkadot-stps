use crate::prelude::*;

#[cfg(not(feature = "spammer"))]
async fn run_spammer(_args: SpammerArgs) -> Result<(), CliError> {
    panic!("Spammer feature not enabled");
}

#[cfg(feature = "spammer")]
async fn run_spammer(args: SpammerArgs) -> Result<(), CliError> {
    let parameters = SpammerParameters::try_from(args)?;
    let mut spammer = Spammer::bootstrap(parameters).await?;
    spammer.run().await.map_err(CliError::CoreError)
}

async fn run_subcommand(command: Command) -> Result<(), CliError> {
    match command {
        Command::Spammer(spammer_args) => run_spammer(spammer_args).await,
    }
}

pub async fn run(cli_args: CliArgs) {
    match run_subcommand(cli_args.command).await {
        Ok(_) => info!("{} ran successfully", BINARY_NAME),
        Err(e) => error!("Error running {}: {}", BINARY_NAME, e),
    }
}
