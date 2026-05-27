use anyhow::{anyhow, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "importer")]
#[command(about = "Portal event importer and snapshot converter")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Subcommands>,

    #[command(flatten)]
    pub legacy: LegacyArgs,
}

#[derive(Debug, Subcommand)]
pub enum Subcommands {
    Import(ImportArgs),
    Convert(ConvertArgs),
}

#[derive(Debug)]
pub enum Command {
    Import(ImportArgs),
    Convert(ConvertArgs),
}

impl Cli {
    pub fn into_command(self) -> Result<Command> {
        if let Some(command) = self.command {
            return Ok(match command {
                Subcommands::Import(args) => Command::Import(args),
                Subcommands::Convert(args) => Command::Convert(args),
            });
        }

        if self.legacy.convert {
            Ok(Command::Convert(ConvertArgs {
                filename: required(self.legacy.filename, "--filename")?,
                target_host_id: required(self.legacy.target_host_id, "--targetHostId")?,
                admin_user_id: required(self.legacy.admin_user_id, "--adminUserId")?,
                output: self.legacy.output,
                schema_source: self.legacy.schema_source,
            }))
        } else {
            Ok(Command::Import(ImportArgs {
                filename: required(self.legacy.filename, "--filename")?,
                replacement: self.legacy.replacement,
                enrichment: self.legacy.enrichment,
                dry_run: self.legacy.dry_run,
                fail_fast: self.legacy.fail_fast,
                batch_size: self.legacy.batch_size,
                summary_json: self.legacy.summary_json,
            }))
        }
    }
}

fn required(value: Option<String>, name: &str) -> Result<String> {
    value.ok_or_else(|| anyhow!("{name} is required"))
}

#[derive(Debug, Clone, Args)]
pub struct ImportArgs {
    #[arg(long, short = 'f')]
    pub filename: String,

    #[arg(long, short = 'r')]
    pub replacement: Option<String>,

    #[arg(long, short = 'e')]
    pub enrichment: Option<String>,

    #[arg(long)]
    pub dry_run: bool,

    #[arg(long)]
    pub fail_fast: bool,

    #[arg(long, default_value_t = 1)]
    pub batch_size: usize,

    #[arg(long)]
    pub summary_json: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ConvertArgs {
    #[arg(long, short = 'f')]
    pub filename: String,

    #[arg(long = "targetHostId", alias = "target-host-id", short = 't')]
    pub target_host_id: String,

    #[arg(long = "adminUserId", alias = "admin-user-id", short = 'u')]
    pub admin_user_id: String,

    #[arg(long, short = 'o')]
    pub output: Option<String>,

    #[arg(long, value_enum, default_value_t = SchemaSource::Embedded)]
    pub schema_source: SchemaSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SchemaSource {
    Embedded,
    Database,
}

impl SchemaSource {
    pub fn requires_database(self) -> bool {
        matches!(self, SchemaSource::Database)
    }
}

#[derive(Debug, Clone, Args)]
pub struct LegacyArgs {
    #[arg(long, short = 'f')]
    pub filename: Option<String>,

    #[arg(long, short = 'r')]
    pub replacement: Option<String>,

    #[arg(long, short = 'e')]
    pub enrichment: Option<String>,

    #[arg(long, short = 'c')]
    pub convert: bool,

    #[arg(long = "targetHostId", alias = "target-host-id", short = 't')]
    pub target_host_id: Option<String>,

    #[arg(long = "adminUserId", alias = "admin-user-id", short = 'u')]
    pub admin_user_id: Option<String>,

    #[arg(long, short = 'o')]
    pub output: Option<String>,

    #[arg(long, value_enum, default_value_t = SchemaSource::Embedded)]
    pub schema_source: SchemaSource,

    #[arg(long)]
    pub dry_run: bool,

    #[arg(long)]
    pub fail_fast: bool,

    #[arg(long, default_value_t = 1)]
    pub batch_size: usize,

    #[arg(long)]
    pub summary_json: bool,
}
