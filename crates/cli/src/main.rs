//! Strata CLI — Redis-inspired CLI for the Strata database.
//!
//! Two modes:
//! - **Shell mode**: `strata [flags] COMMAND` — single command, exit
//! - **REPL mode**: `strata [flags]` — interactive prompt (if stdin is TTY)
//! - **Pipe mode**: `echo "kv put k v" | strata` — line-by-line from stdin

mod commands;
mod format;
mod parse;
mod repl;
mod state;
mod value;

use std::io::IsTerminal;
use std::process;

use strata_executor::{AccessMode, OpenOptions, Strata};

use commands::build_cli;
use format::{
    format_diff, format_error, format_fork_info, format_merge_info, format_output, OutputMode,
};
use parse::{matches_to_action, BranchOp, CliAction};
use state::SessionState;

fn main() {
    let cli = build_cli();
    let matches = cli.get_matches();

    // Determine output mode
    let output_mode = if matches.get_flag("json") {
        OutputMode::Json
    } else if matches.get_flag("raw") {
        OutputMode::Raw
    } else {
        OutputMode::Human
    };

    // Open database
    let db = match open_database(&matches) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    // Initial branch/space
    let initial_branch = matches
        .get_one::<String>("branch")
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    let initial_space = matches
        .get_one::<String>("space")
        .cloned()
        .unwrap_or_else(|| "default".to_string());

    let mut state = SessionState::new(db, initial_branch, initial_space);

    // Dispatch mode
    if matches.subcommand().is_some() {
        // Shell mode: parse, execute, format, exit
        let exit_code = run_shell_mode(&matches, &mut state, output_mode);
        process::exit(exit_code);
    } else if std::io::stdin().is_terminal() {
        // REPL mode
        repl::run_repl(&mut state, output_mode);
    } else {
        // Pipe mode
        let exit_code = repl::run_pipe(&mut state, output_mode);
        process::exit(exit_code);
    }
}

fn open_database(matches: &clap::ArgMatches) -> Result<Strata, String> {
    let read_only = matches.get_flag("read-only");
    let use_cache = matches.get_flag("cache");

    if use_cache {
        Strata::cache().map_err(|e| format!("Failed to open cache database: {}", e))
    } else {
        let path = matches
            .get_one::<String>("db")
            .map(|s| s.as_str())
            .unwrap_or(".strata");

        if read_only {
            let opts = OpenOptions::new().access_mode(AccessMode::ReadOnly);
            Strata::open_with(path, opts)
                .map_err(|e| format!("Failed to open database (read-only): {}", e))
        } else {
            Strata::open(path).map_err(|e| format!("Failed to open database: {}", e))
        }
    }
}

fn run_shell_mode(
    matches: &clap::ArgMatches,
    state: &mut SessionState,
    mode: OutputMode,
) -> i32 {
    match matches_to_action(matches, state) {
        Ok(CliAction::Execute(cmd)) => match state.execute(cmd) {
            Ok(output) => {
                let formatted = format_output(&output, mode);
                if !formatted.is_empty() {
                    println!("{}", formatted);
                }
                0
            }
            Err(e) => {
                eprintln!("{}", format_error(&e, mode));
                1
            }
        },
        Ok(CliAction::BranchOp(op)) => match op {
            BranchOp::Fork { destination } => match state.fork_branch(&destination) {
                Ok(info) => {
                    println!("{}", format_fork_info(&info, mode));
                    0
                }
                Err(e) => {
                    eprintln!("{}", format_error(&e, mode));
                    1
                }
            },
            BranchOp::Diff {
                branch_a,
                branch_b,
            } => match state.diff_branches(&branch_a, &branch_b) {
                Ok(diff) => {
                    println!("{}", format_diff(&diff, mode));
                    0
                }
                Err(e) => {
                    eprintln!("{}", format_error(&e, mode));
                    1
                }
            },
            BranchOp::Merge { source, strategy } => match state.merge_branch(&source, strategy) {
                Ok(info) => {
                    println!("{}", format_merge_info(&info, mode));
                    0
                }
                Err(e) => {
                    eprintln!("{}", format_error(&e, mode));
                    1
                }
            },
        },
        Ok(CliAction::Meta(_)) => {
            eprintln!("(error) Meta-commands are only available in REPL mode");
            1
        }
        Err(e) => {
            eprintln!("(error) {}", e);
            1
        }
    }
}
