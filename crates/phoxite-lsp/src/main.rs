//! `phoxite-lsp`: a language server for PHP and Laravel.
//!
//! It speaks LSP over stdin/stdout. Logs go to stderr (`PHOXITE_LOG`
//! sets the filter, `info` by default).

mod analyzers;
mod commands;
mod config;
mod convert;
mod handlers;
mod laravel;
mod refactor;
mod schema;
mod server;
mod state;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("phoxite-lsp {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "phoxite-lsp {}\nA language server for PHP and Laravel. It talks LSP over stdin/stdout.\n\n\
             Options:\n  --stdio     use stdin/stdout (the default)\n  --version   print the version\n\n\
             Environment:\n  PHOXITE_LOG   log filter for stderr, e.g. `debug` (default `info`)",
            env!("CARGO_PKG_VERSION")
        );
        return ExitCode::SUCCESS;
    }
    let filter = tracing_subscriber::EnvFilter::try_from_env("PHOXITE_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_writer(std::io::stderr).with_ansi(false).with_env_filter(filter).init();

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
