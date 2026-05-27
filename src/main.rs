use anyhow::Result;
use clap::Parser;
use importer::cli::{Cli, Command};
use importer::config::AppConfig;
use importer::db::Database;
use importer::importer::{run_import, ImportSummary};
use importer::snapshot::converter::run_convert;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    init_tracing();

    let exit_code = match run().await {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err:#}");
            4
        }
    };

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

async fn run() -> Result<i32> {
    let cli = Cli::parse();
    let command = cli.into_command()?;

    match command {
        Command::Import(args) => {
            let config = AppConfig::from_env();
            let db = if args.dry_run {
                None
            } else {
                Some(
                    Database::connect(config.database_url_required()?, config.max_connections)
                        .await?,
                )
            };
            let summary = run_import(args, db.as_ref()).await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            Ok(import_exit_code(&summary))
        }
        Command::Convert(args) => {
            let config = AppConfig::from_env();
            let db = if args.schema_source.requires_database() {
                Some(
                    Database::connect(config.database_url_required()?, config.max_connections)
                        .await?,
                )
            } else {
                None
            };
            run_convert(args, db.as_ref()).await?;
            Ok(0)
        }
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

fn import_exit_code(summary: &ImportSummary) -> i32 {
    if summary.failed > 0 {
        3
    } else if summary.skipped_duplicate_input > 0
        || summary.skipped_existing_target > 0
        || summary.skipped_exact_duplicate > 0
    {
        2
    } else {
        0
    }
}
