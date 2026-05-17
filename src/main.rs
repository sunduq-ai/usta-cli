//! `usta` — multi-stack project scaffolder.
//!
//! This crate is the only composition root. Concrete adapters from
//! `crate::adapters` are wired into use cases from `crate::app` here, and
//! exposed through `clap` subcommands.

#![forbid(unsafe_code)]

// Layered engine, all in one crate. The hexagonal architecture is now
// enforced by module discipline rather than by Cargo's crate graph — see
// `AGENTS.md` for the dependency rules each module is expected to honor.
mod adapters;
mod app;
mod commands;
mod core;
mod ports;
mod wiring;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

/// Multi-stack project scaffolder with deterministic template extraction.
#[derive(Debug, Parser)]
#[command(
    name = "usta",
    version,
    about,
    long_about = None,
    propagate_version = true,
    arg_required_else_help = false,
)]
struct Cli {
    /// Increase log verbosity (`-v`, `-vv`).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Suppress non-error output.
    #[arg(short, long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Scaffold a new project from a template.
    New(commands::new::NewArgs),

    /// Generate a template from an existing repository (deterministic, no LLM).
    Extract(commands::extract::ExtractArgs),

    /// Re-render template against stored answers (3-way merge).
    Update(commands::update::UpdateArgs),

    /// Apply a single feature to an existing generated project.
    Add(commands::add::AddArgs),

    /// Detect drift in template-managed files.
    Verify(commands::verify::VerifyArgs),

    /// Discover, list, and inspect templates and features.
    #[command(subcommand)]
    List(commands::list::ListCmd),

    /// Verify required tools and environment.
    Doctor(commands::doctor::DoctorArgs),

    /// Discover community templates via GitHub topic.
    Search(commands::search::SearchArgs),

    /// Install a community template into the local cache.
    Install(commands::install::InstallArgs),

    /// Generate shell completions.
    Completions(commands::completions::CompletionsArgs),

    /// Replace this binary with the latest release.
    SelfUpdate(commands::self_update::SelfUpdateArgs),

    /// Emit JSON Schema for template / feature manifests.
    Schema(commands::schema::SchemaArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    init_logging(cli.verbose, cli.quiet);

    match cli.command {
        None => {
            // No subcommand: print help and exit 0 (clap convention via `arg_required_else_help`
            // is to exit 2; we want 0 so `usta` alone is friendly).
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
            Ok(())
        }
        Some(Command::New(args)) => commands::new::run(args),
        Some(Command::Extract(args)) => commands::extract::run(args),
        Some(Command::Update(args)) => commands::update::run(args),
        Some(Command::Add(args)) => commands::add::run(args),
        Some(Command::Verify(args)) => commands::verify::run(args),
        Some(Command::List(cmd)) => commands::list::run(cmd),
        Some(Command::Doctor(args)) => commands::doctor::run(args),
        Some(Command::Search(args)) => commands::search::run(args),
        Some(Command::Install(args)) => commands::install::run(args),
        Some(Command::Completions(args)) => {
            use clap::CommandFactory;
            let mut cmd = Cli::command();
            commands::completions::run(args, &mut cmd)
        }
        Some(Command::SelfUpdate(args)) => commands::self_update::run(args),
        Some(Command::Schema(args)) => commands::schema::run(args),
    }
}

fn init_logging(verbose: u8, quiet: bool) {
    let level = if quiet {
        "error"
    } else {
        match verbose {
            0 => "info",
            1 => "debug",
            _ => "trace",
        }
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .try_init();
}
