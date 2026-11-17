//! `phoxite-lsp`: a language server for PHP and Laravel.
//!
//! It speaks LSP over stdin/stdout. Logs go to stderr (`PHOXITE_LOG`
//! sets the filter, `info` by default).
//!
//! `phoxite-lsp check` inspects a project from the command line instead
//! ([`check`], `warn` by default: its stdout is the report).

mod analyzers;
mod check;
mod cli;
mod commands;
mod config;
mod convert;
mod handlers;
mod laravel;
mod pint;
mod refactor;
mod report;
mod schema;
mod server;
mod state;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let version = concat!("phoxite-lsp ", env!("CARGO_PKG_VERSION"));
    let command = match cli::parse(&args) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("phoxite-lsp check: {message}\n\n{}", cli::CHECK_USAGE);
            return ExitCode::from(check::USAGE);
        }
    };
    let log = |default: &str| {
        let filter = tracing_subscriber::EnvFilter::try_from_env("PHOXITE_LOG")
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default));
        tracing_subscriber::fmt().with_writer(std::io::stderr).with_ansi(false).with_env_filter(filter).init();
    };
    match command {
        cli::Command::Version => {
            println!("{version}");
            return ExitCode::SUCCESS;
        }
        cli::Command::Help => {
            println!("{version}\n{}", cli::USAGE);
            return ExitCode::SUCCESS;
        }
        cli::Command::CheckHelp => {
            println!("{}", cli::CHECK_USAGE);
            return ExitCode::SUCCESS;
        }
        cli::Command::Check(options) => {
            log("warn");
            return ExitCode::from(check::run(&options));
        }
        cli::Command::Serve => log("info"),
    }

    let (connection, io_threads) = lsp_server::Connection::stdio();
    let result = server::run(connection);
    if let Err(e) = io_threads.join() {
        tracing::error!("io: {e}");
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
