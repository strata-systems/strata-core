//! ArgMatches → Command/BranchOp/MetaCommand conversion.
//!
//! Translates clap's parsed arguments into the appropriate action:
//! - Standard commands → `CliAction::Execute(Command)`
//! - Branch power ops → `CliAction::BranchOp`
//! - REPL meta-commands → `CliAction::Meta`

use clap::ArgMatches;
use strata_executor::{
    BranchId, BatchVectorEntry, Command, DistanceMetric, MergeStrategy, MetadataFilter, TxnOptions,
};

use crate::state::SessionState;
use crate::value::{parse_json_value, parse_value, parse_vector};

/// The result of parsing user input.
#[allow(dead_code)]
pub enum CliAction {
    /// A standard command to execute via Session.
    Execute(Command),
    /// A branch power-API operation (fork/diff/merge).
    BranchOp(BranchOp),
    /// A REPL-only meta-command.
    Meta(MetaCommand),
}

/// Branch operations that bypass the Command enum.
pub enum BranchOp {
    Fork { destination: String },
    Diff { branch_a: String, branch_b: String },
    Merge { source: String, strategy: MergeStrategy },
}

/// REPL meta-commands.
pub enum MetaCommand {
    Use { branch: String, space: Option<String> },
    Help { command: Option<String> },
    Quit,
    Clear,
}

/// Check for REPL meta-commands before delegating to clap.
///
/// Returns `Some(MetaCommand)` if the line is a meta-command, `None` otherwise.
pub fn check_meta_command(line: &str) -> Option<MetaCommand> {
    let trimmed = line.trim();
    let mut parts = trimmed.splitn(3, char::is_whitespace);
    let cmd = parts.next()?;

    match cmd {
        "quit" | "exit" => Some(MetaCommand::Quit),
        "clear" => Some(MetaCommand::Clear),
        "help" => {
            let command = parts.next().map(|s| s.trim().to_string());
            Some(MetaCommand::Help { command })
        }
        "use" => {
            let branch = parts.next()?.trim().to_string();
            let space = parts.next().map(|s| s.trim().to_string());
            Some(MetaCommand::Use { branch, space })
        }
        _ => None,
    }
}

/// Convert clap ArgMatches into a CliAction.
pub fn matches_to_action(matches: &ArgMatches, state: &SessionState) -> Result<CliAction, String> {
    let (sub_name, sub_matches) = matches
        .subcommand()
        .ok_or_else(|| "No command provided".to_string())?;

    match sub_name {
        "kv" => parse_kv(sub_matches, state),
        "json" => parse_json(sub_matches, state),
        "event" => parse_event(sub_matches, state),
        "state" => parse_state(sub_matches, state),
        "vector" => parse_vector_cmd(sub_matches, state),
        "branch" => parse_branch(sub_matches, state),
        "space" => parse_space(sub_matches, state),
        "begin" => parse_begin(sub_matches, state),
        "commit" => Ok(CliAction::Execute(Command::TxnCommit)),
        "rollback" => Ok(CliAction::Execute(Command::TxnRollback)),
        "txn" => parse_txn(sub_matches),
        "ping" => Ok(CliAction::Execute(Command::Ping)),
        "info" => Ok(CliAction::Execute(Command::Info)),
        "flush" => Ok(CliAction::Execute(Command::Flush)),
        "compact" => Ok(CliAction::Execute(Command::Compact)),
        "search" => parse_search(sub_matches, state),
        other => Err(format!("Unknown command: {}", other)),
    }
}

// =========================================================================
// Branch/space helpers
// =========================================================================

fn branch(state: &SessionState) -> Option<BranchId> {
    Some(BranchId::from(state.branch()))
}

fn space(state: &SessionState) -> Option<String> {
    Some(state.space().to_string())
}

// =========================================================================
// KV
// =========================================================================

fn parse_kv(matches: &ArgMatches, state: &SessionState) -> Result<CliAction, String> {
    let (sub, m) = matches.subcommand().ok_or("No kv subcommand")?;
    match sub {
        "put" => {
            let key = m.get_one::<String>("key").unwrap().clone();
            let raw = m.get_one::<String>("value").unwrap();
            let value = parse_value(raw);
            Ok(CliAction::Execute(Command::KvPut {
                branch: branch(state),
                space: space(state),
                key,
                value,
            }))
        }
        "get" => {
            let key = m.get_one::<String>("key").unwrap().clone();
            Ok(CliAction::Execute(Command::KvGet {
                branch: branch(state),
                space: space(state),
                key,
            }))
        }
        "del" => {
            let key = m.get_one::<String>("key").unwrap().clone();
            Ok(CliAction::Execute(Command::KvDelete {
                branch: branch(state),
                space: space(state),
                key,
            }))
        }
        "list" => {
            let prefix = m.get_one::<String>("prefix").cloned();
            let limit = m
                .get_one::<String>("limit")
                .map(|s| s.parse::<u64>())
                .transpose()
                .map_err(|e| format!("Invalid limit: {}", e))?;
            let cursor = m.get_one::<String>("cursor").cloned();
            Ok(CliAction::Execute(Command::KvList {
                branch: branch(state),
                space: space(state),
                prefix,
                cursor,
                limit,
            }))
        }
        "history" => {
            let key = m.get_one::<String>("key").unwrap().clone();
            Ok(CliAction::Execute(Command::KvGetv {
                branch: branch(state),
                space: space(state),
                key,
            }))
        }
        other => Err(format!("Unknown kv subcommand: {}", other)),
    }
}

// =========================================================================
// JSON
// =========================================================================

fn parse_json(matches: &ArgMatches, state: &SessionState) -> Result<CliAction, String> {
    let (sub, m) = matches.subcommand().ok_or("No json subcommand")?;
    match sub {
        "set" => {
            let key = m.get_one::<String>("key").unwrap().clone();
            let path = m.get_one::<String>("path").unwrap().clone();
            let raw = m.get_one::<String>("value").unwrap();
            let value = parse_json_value(raw)?;
            Ok(CliAction::Execute(Command::JsonSet {
                branch: branch(state),
                space: space(state),
                key,
                path,
                value,
            }))
        }
        "get" => {
            let key = m.get_one::<String>("key").unwrap().clone();
            let path = m.get_one::<String>("path").unwrap().clone();
            Ok(CliAction::Execute(Command::JsonGet {
                branch: branch(state),
                space: space(state),
                key,
                path,
            }))
        }
        "del" => {
            let key = m.get_one::<String>("key").unwrap().clone();
            let path = m.get_one::<String>("path").unwrap().clone();
            Ok(CliAction::Execute(Command::JsonDelete {
                branch: branch(state),
                space: space(state),
                key,
                path,
            }))
        }
        "list" => {
            let prefix = m.get_one::<String>("prefix").cloned();
            let cursor = m.get_one::<String>("cursor").cloned();
            let limit = m
                .get_one::<String>("limit")
                .map(|s| s.parse::<u64>())
                .transpose()
                .map_err(|e| format!("Invalid limit: {}", e))?
                .unwrap_or(100);
            Ok(CliAction::Execute(Command::JsonList {
                branch: branch(state),
                space: space(state),
                prefix,
                cursor,
                limit,
            }))
        }
        "history" => {
            let key = m.get_one::<String>("key").unwrap().clone();
            Ok(CliAction::Execute(Command::JsonGetv {
                branch: branch(state),
                space: space(state),
                key,
            }))
        }
        other => Err(format!("Unknown json subcommand: {}", other)),
    }
}

// =========================================================================
// Event
// =========================================================================

fn parse_event(matches: &ArgMatches, state: &SessionState) -> Result<CliAction, String> {
    let (sub, m) = matches.subcommand().ok_or("No event subcommand")?;
    match sub {
        "append" => {
            let event_type = m.get_one::<String>("type").unwrap().clone();
            let raw = m.get_one::<String>("payload").unwrap();
            let payload = parse_json_value(raw)?;
            Ok(CliAction::Execute(Command::EventAppend {
                branch: branch(state),
                space: space(state),
                event_type,
                payload,
            }))
        }
        "get" => {
            let sequence = m
                .get_one::<String>("sequence")
                .unwrap()
                .parse::<u64>()
                .map_err(|e| format!("Invalid sequence: {}", e))?;
            Ok(CliAction::Execute(Command::EventGet {
                branch: branch(state),
                space: space(state),
                sequence,
            }))
        }
        "list" => {
            let event_type = m.get_one::<String>("type").unwrap().clone();
            let limit = m
                .get_one::<String>("limit")
                .map(|s| s.parse::<u64>())
                .transpose()
                .map_err(|e| format!("Invalid limit: {}", e))?;
            let after_sequence = m
                .get_one::<String>("after")
                .map(|s| s.parse::<u64>())
                .transpose()
                .map_err(|e| format!("Invalid after: {}", e))?;
            Ok(CliAction::Execute(Command::EventGetByType {
                branch: branch(state),
                space: space(state),
                event_type,
                limit,
                after_sequence,
            }))
        }
        "len" => Ok(CliAction::Execute(Command::EventLen {
            branch: branch(state),
            space: space(state),
        })),
        other => Err(format!("Unknown event subcommand: {}", other)),
    }
}

// =========================================================================
// State
// =========================================================================

fn parse_state(matches: &ArgMatches, state: &SessionState) -> Result<CliAction, String> {
    let (sub, m) = matches.subcommand().ok_or("No state subcommand")?;
    match sub {
        "set" => {
            let cell = m.get_one::<String>("cell").unwrap().clone();
            let raw = m.get_one::<String>("value").unwrap();
            let value = parse_value(raw);
            Ok(CliAction::Execute(Command::StateSet {
                branch: branch(state),
                space: space(state),
                cell,
                value,
            }))
        }
        "get" => {
            let cell = m.get_one::<String>("cell").unwrap().clone();
            Ok(CliAction::Execute(Command::StateGet {
                branch: branch(state),
                space: space(state),
                cell,
            }))
        }
        "del" => {
            let cell = m.get_one::<String>("cell").unwrap().clone();
            Ok(CliAction::Execute(Command::StateDelete {
                branch: branch(state),
                space: space(state),
                cell,
            }))
        }
        "init" => {
            let cell = m.get_one::<String>("cell").unwrap().clone();
            let raw = m.get_one::<String>("value").unwrap();
            let value = parse_value(raw);
            Ok(CliAction::Execute(Command::StateInit {
                branch: branch(state),
                space: space(state),
                cell,
                value,
            }))
        }
        "cas" => {
            let cell = m.get_one::<String>("cell").unwrap().clone();
            let expected_str = m.get_one::<String>("expected").unwrap();
            let expected_counter = if expected_str == "none" {
                None
            } else {
                Some(
                    expected_str
                        .parse::<u64>()
                        .map_err(|e| format!("Invalid expected version: {}", e))?,
                )
            };
            let raw = m.get_one::<String>("value").unwrap();
            let value = parse_value(raw);
            Ok(CliAction::Execute(Command::StateCas {
                branch: branch(state),
                space: space(state),
                cell,
                expected_counter,
                value,
            }))
        }
        "list" => {
            let prefix = m.get_one::<String>("prefix").cloned();
            Ok(CliAction::Execute(Command::StateList {
                branch: branch(state),
                space: space(state),
                prefix,
            }))
        }
        "history" => {
            let cell = m.get_one::<String>("cell").unwrap().clone();
            Ok(CliAction::Execute(Command::StateGetv {
                branch: branch(state),
                space: space(state),
                cell,
            }))
        }
        other => Err(format!("Unknown state subcommand: {}", other)),
    }
}

// =========================================================================
// Vector
// =========================================================================

fn parse_metric(s: &str) -> Result<DistanceMetric, String> {
    match s.to_lowercase().as_str() {
        "cosine" => Ok(DistanceMetric::Cosine),
        "euclidean" => Ok(DistanceMetric::Euclidean),
        "dotproduct" | "dot_product" | "dot" => Ok(DistanceMetric::DotProduct),
        other => Err(format!("Unknown metric: {}. Use cosine, euclidean, or dotproduct", other)),
    }
}

fn parse_vector_cmd(matches: &ArgMatches, state: &SessionState) -> Result<CliAction, String> {
    let (sub, m) = matches.subcommand().ok_or("No vector subcommand")?;
    match sub {
        "upsert" => {
            let collection = m.get_one::<String>("collection").unwrap().clone();
            let key = m.get_one::<String>("key").unwrap().clone();
            let vector = parse_vector(m.get_one::<String>("vector").unwrap())?;
            let metadata = m
                .get_one::<String>("metadata")
                .map(|s| parse_json_value(s))
                .transpose()?;
            Ok(CliAction::Execute(Command::VectorUpsert {
                branch: branch(state),
                space: space(state),
                collection,
                key,
                vector,
                metadata,
            }))
        }
        "get" => {
            let collection = m.get_one::<String>("collection").unwrap().clone();
            let key = m.get_one::<String>("key").unwrap().clone();
            Ok(CliAction::Execute(Command::VectorGet {
                branch: branch(state),
                space: space(state),
                collection,
                key,
            }))
        }
        "del" => {
            let collection = m.get_one::<String>("collection").unwrap().clone();
            let key = m.get_one::<String>("key").unwrap().clone();
            Ok(CliAction::Execute(Command::VectorDelete {
                branch: branch(state),
                space: space(state),
                collection,
                key,
            }))
        }
        "search" => {
            let collection = m.get_one::<String>("collection").unwrap().clone();
            let query = parse_vector(m.get_one::<String>("query").unwrap())?;
            let k = m
                .get_one::<String>("k")
                .unwrap()
                .parse::<u64>()
                .map_err(|e| format!("Invalid k: {}", e))?;
            let metric = m
                .get_one::<String>("metric")
                .map(|s| parse_metric(s))
                .transpose()?;
            let filter = m
                .get_one::<String>("filter")
                .map(|s| -> Result<Vec<MetadataFilter>, String> {
                    serde_json::from_str(s).map_err(|e| format!("Invalid filter JSON: {}", e))
                })
                .transpose()?;
            Ok(CliAction::Execute(Command::VectorSearch {
                branch: branch(state),
                space: space(state),
                collection,
                query,
                k,
                filter,
                metric,
            }))
        }
        "create" => {
            let collection = m.get_one::<String>("name").unwrap().clone();
            let dimension = m
                .get_one::<String>("dim")
                .unwrap()
                .parse::<u64>()
                .map_err(|e| format!("Invalid dimension: {}", e))?;
            let metric = parse_metric(m.get_one::<String>("metric").unwrap())?;
            Ok(CliAction::Execute(Command::VectorCreateCollection {
                branch: branch(state),
                space: space(state),
                collection,
                dimension,
                metric,
            }))
        }
        "drop" => {
            let collection = m.get_one::<String>("name").unwrap().clone();
            Ok(CliAction::Execute(Command::VectorDeleteCollection {
                branch: branch(state),
                space: space(state),
                collection,
            }))
        }
        "collections" => Ok(CliAction::Execute(Command::VectorListCollections {
            branch: branch(state),
            space: space(state),
        })),
        "stats" => {
            let collection = m.get_one::<String>("collection").unwrap().clone();
            Ok(CliAction::Execute(Command::VectorCollectionStats {
                branch: branch(state),
                space: space(state),
                collection,
            }))
        }
        "batch-upsert" => {
            let collection = m.get_one::<String>("collection").unwrap().clone();
            let raw = m.get_one::<String>("json").unwrap();
            let entries: Vec<BatchVectorEntry> =
                serde_json::from_str(raw).map_err(|e| format!("Invalid batch JSON: {}", e))?;
            Ok(CliAction::Execute(Command::VectorBatchUpsert {
                branch: branch(state),
                space: space(state),
                collection,
                entries,
            }))
        }
        other => Err(format!("Unknown vector subcommand: {}", other)),
    }
}

// =========================================================================
// Branch
// =========================================================================

fn parse_branch(matches: &ArgMatches, _state: &SessionState) -> Result<CliAction, String> {
    let (sub, m) = matches.subcommand().ok_or("No branch subcommand")?;
    match sub {
        "create" => {
            let branch_id = m.get_one::<String>("name").cloned();
            Ok(CliAction::Execute(Command::BranchCreate {
                branch_id,
                metadata: None,
            }))
        }
        "info" => {
            let name = m.get_one::<String>("name").unwrap().clone();
            Ok(CliAction::Execute(Command::BranchGet {
                branch: BranchId::from(name),
            }))
        }
        "list" => {
            let limit = m
                .get_one::<String>("limit")
                .map(|s| s.parse::<u64>())
                .transpose()
                .map_err(|e| format!("Invalid limit: {}", e))?;
            Ok(CliAction::Execute(Command::BranchList {
                state: None,
                limit,
                offset: None,
            }))
        }
        "exists" => {
            let name = m.get_one::<String>("name").unwrap().clone();
            Ok(CliAction::Execute(Command::BranchExists {
                branch: BranchId::from(name),
            }))
        }
        "del" => {
            let name = m.get_one::<String>("name").unwrap().clone();
            Ok(CliAction::Execute(Command::BranchDelete {
                branch: BranchId::from(name),
            }))
        }
        "fork" => {
            let destination = m.get_one::<String>("dest").unwrap().clone();
            Ok(CliAction::BranchOp(BranchOp::Fork { destination }))
        }
        "diff" => {
            let branch_a = m.get_one::<String>("a").unwrap().clone();
            let branch_b = m.get_one::<String>("b").unwrap().clone();
            Ok(CliAction::BranchOp(BranchOp::Diff { branch_a, branch_b }))
        }
        "merge" => {
            let source = m.get_one::<String>("source").unwrap().clone();
            let strategy = match m.get_one::<String>("strategy").map(|s| s.as_str()) {
                Some("strict") => MergeStrategy::Strict,
                _ => MergeStrategy::LastWriterWins,
            };
            Ok(CliAction::BranchOp(BranchOp::Merge { source, strategy }))
        }
        "export" => {
            let branch_id = m.get_one::<String>("branch").unwrap().clone();
            let path = m.get_one::<String>("path").unwrap().clone();
            Ok(CliAction::Execute(Command::BranchExport { branch_id, path }))
        }
        "import" => {
            let path = m.get_one::<String>("path").unwrap().clone();
            Ok(CliAction::Execute(Command::BranchImport { path }))
        }
        "validate" => {
            let path = m.get_one::<String>("path").unwrap().clone();
            Ok(CliAction::Execute(Command::BranchBundleValidate { path }))
        }
        other => Err(format!("Unknown branch subcommand: {}", other)),
    }
}

// =========================================================================
// Space
// =========================================================================

fn parse_space(matches: &ArgMatches, state: &SessionState) -> Result<CliAction, String> {
    let (sub, m) = matches.subcommand().ok_or("No space subcommand")?;
    match sub {
        "list" => Ok(CliAction::Execute(Command::SpaceList {
            branch: branch(state),
        })),
        "create" => {
            let name = m.get_one::<String>("name").unwrap().clone();
            Ok(CliAction::Execute(Command::SpaceCreate {
                branch: branch(state),
                space: name,
            }))
        }
        "del" => {
            let name = m.get_one::<String>("name").unwrap().clone();
            let force = m.get_flag("force");
            Ok(CliAction::Execute(Command::SpaceDelete {
                branch: branch(state),
                space: name,
                force,
            }))
        }
        "exists" => {
            let name = m.get_one::<String>("name").unwrap().clone();
            Ok(CliAction::Execute(Command::SpaceExists {
                branch: branch(state),
                space: name,
            }))
        }
        other => Err(format!("Unknown space subcommand: {}", other)),
    }
}

// =========================================================================
// Transaction
// =========================================================================

fn parse_begin(matches: &ArgMatches, state: &SessionState) -> Result<CliAction, String> {
    let read_only = matches.get_flag("txn-read-only");
    Ok(CliAction::Execute(Command::TxnBegin {
        branch: branch(state),
        options: Some(TxnOptions { read_only }),
    }))
}

fn parse_txn(matches: &ArgMatches) -> Result<CliAction, String> {
    let (sub, _) = matches.subcommand().ok_or("No txn subcommand")?;
    match sub {
        "info" => Ok(CliAction::Execute(Command::TxnInfo)),
        "active" => Ok(CliAction::Execute(Command::TxnIsActive)),
        other => Err(format!("Unknown txn subcommand: {}", other)),
    }
}

// =========================================================================
// Search
// =========================================================================

fn parse_search(matches: &ArgMatches, state: &SessionState) -> Result<CliAction, String> {
    let query = matches.get_one::<String>("query").unwrap().clone();
    let k = matches
        .get_one::<String>("k")
        .map(|s| s.parse::<u64>())
        .transpose()
        .map_err(|e| format!("Invalid k: {}", e))?;
    let primitives = matches
        .get_one::<String>("primitives")
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect());
    Ok(CliAction::Execute(Command::Search {
        branch: branch(state),
        space: space(state),
        query,
        k,
        primitives,
    }))
}
