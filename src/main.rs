use clap::{Parser, Subcommand};
use std::fs::File;

use heap_snapshot::parser;
use heap_snapshot::print::{self, EdgeWindow, ExpandMap, GroupExpandMap, GroupWindow};
use heap_snapshot::snapshot::{self, HeapSnapshot};
use heap_snapshot::tui;
use heap_snapshot::types::{self, NodeId};

#[derive(Parser)]
#[command(about = "Heap snapshot CLI analyzer")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Interactive TUI viewer (default)
    View {
        /// Main .heapsnapshot file
        main: String,
        /// Optional comparison .heapsnapshot files for diff (can specify multiple)
        compare: Vec<String>,
    },
    /// Print summary table
    Summary {
        /// Path to .heapsnapshot file
        file: String,
        /// Max depth for expanded nodes
        #[arg(long, default_value = "0")]
        depth: usize,
        /// Expand a constructor group: NAME, NAME:start, or NAME:start:count (can be repeated)
        #[arg(short = 'g', long, value_name = "NAME")]
        expand_group: Vec<String>,
        /// Expand a node's edges: @id, @id:start, or @id:start:count (can be repeated)
        #[arg(short = 'e', long, value_name = "ID")]
        expand: Vec<String>,
    },
    /// Show retainers for an object
    Retainers {
        /// Path to .heapsnapshot file
        file: String,
        /// Object ID (e.g. @3005313 or 3005313)
        object_id: String,
        /// Max depth for retainer tree
        #[arg(long, default_value = "0")]
        depth: usize,
        /// Width of the Object column
        #[arg(long = "column-length")]
        column_length: Option<usize>,
        /// Max auto-expansion search depth for GC-root retainer paths
        #[arg(long = "max-depth", default_value = "20")]
        max_expand_depth: usize,
        /// Max nodes visited during auto-expansion search
        #[arg(long = "max-nodes", default_value = "2000")]
        max_expand_nodes: usize,
        /// Expand a node's edges: @id, @id:start, or @id:start:count (can be repeated)
        #[arg(short = 'e', long, value_name = "ID")]
        expand: Vec<String>,
    },
    /// Show containment tree
    Containment {
        /// Path to .heapsnapshot file
        file: String,
        /// Object ID (optional, defaults to root)
        object_id: Option<String>,
        /// Max depth for containment tree
        #[arg(long, default_value = "0")]
        depth: usize,
        /// Expand a node's edges: @id, @id:start, or @id:start:count (can be repeated)
        #[arg(short = 'e', long, value_name = "ID")]
        expand: Vec<String>,
    },
    /// Dump native context info
    Contexts {
        /// Path to .heapsnapshot file
        file: String,
    },
    /// Print stack roots (Stack roots and C++ native stack roots)
    Stack {
        /// Path to .heapsnapshot file
        file: String,
        /// Only show objects with at least this reachable size (in MB)
        #[arg(long, value_name = "MB")]
        minimum_reachable_size: Option<f64>,
    },
    /// Show unreachable objects (not reachable from GC roots)
    Unreachable {
        /// Path to .heapsnapshot file
        file: String,
        /// Show only fully unreachable objects (distance U), excluding those
        /// only reachable from other unreachable objects (U+1, U+2, …)
        #[arg(long)]
        full: bool,
    },
    /// Compare two heap snapshots
    Diff {
        /// Main snapshot
        main: String,
        /// Baseline snapshot to compare against
        compare: String,
        /// Expand a constructor group: NAME, NAME:start, or NAME:start:count (can be repeated)
        #[arg(short = 'g', long, value_name = "NAME")]
        expand_group: Vec<String>,
        /// Expand a node's edges: @id, @id:start, or @id:start:count (can be repeated)
        #[arg(short = 'e', long, value_name = "ID")]
        expand: Vec<String>,
    },
}

fn parse_object_id(s: &str) -> Result<NodeId, String> {
    let s = s.strip_prefix('@').unwrap_or(s);
    s.parse::<u64>()
        .map(NodeId)
        .map_err(|_| format!("invalid object ID: {s}"))
}

/// Split a `name:start:count` string from the right, returning the name
/// portion and optional (start, count). Trailing numeric segments are peeled
/// off; everything before them is the name (which may itself contain colons).
fn split_name_window(s: &str) -> (&str, Option<usize>, Option<usize>) {
    let parts: Vec<&str> = s.rsplitn(3, ':').collect();
    match parts.len() {
        1 => (s, None, None),
        2 => {
            if let Ok(n) = parts[0].parse::<usize>() {
                let name_end = s.len() - parts[0].len() - 1;
                (&s[..name_end], Some(n), None)
            } else {
                (s, None, None)
            }
        }
        3 => {
            if let (Ok(start), Ok(count)) = (parts[1].parse::<usize>(), parts[0].parse::<usize>()) {
                let name_end = s.len() - parts[0].len() - parts[1].len() - 2;
                (&s[..name_end], Some(start), Some(count))
            } else if let Ok(n) = parts[0].parse::<usize>() {
                let name_end = s.len() - parts[0].len() - 1;
                (&s[..name_end], Some(n), None)
            } else {
                (s, None, None)
            }
        }
        _ => (s, None, None),
    }
}

/// Parse `--expand` values: `@id`, `@id:start`, or `@id:start:count`
fn parse_expand(expand: &[String]) -> ExpandMap {
    let mut map = ExpandMap::default();
    for s in expand {
        let s = s.strip_prefix('@').unwrap_or(s);
        let (name, start, count) = split_name_window(s);
        let id = name.parse::<u64>().unwrap_or_else(|_| {
            eprintln!("Error: invalid object ID: {name}");
            std::process::exit(1);
        });
        let mut window = EdgeWindow::default();
        if let Some(s) = start {
            window.start = s;
        }
        if let Some(c) = count {
            window.count = c;
        }
        map.insert(NodeId(id), window);
    }
    map
}

/// Parse `--expand-group` values: `NAME`, `NAME:start`, or `NAME:start:count`
fn parse_expand_group(groups: &[String]) -> GroupExpandMap {
    let mut map = GroupExpandMap::default();
    for s in groups {
        let (name, start, count) = split_name_window(s);
        let mut window = GroupWindow::default();
        if let Some(s) = start {
            window.start = s;
        }
        if let Some(c) = count {
            window.count = c;
        }
        map.insert(name.to_string(), window);
    }
    map
}

fn load_snapshot(path: &str) -> HeapSnapshot {
    println!("Reading and parsing {path}...");
    let file = File::open(path).unwrap_or_else(|e| {
        eprintln!("Error reading file: {e}");
        std::process::exit(1);
    });
    let raw = parser::parse(file).unwrap_or_else(|e| {
        eprintln!("Error parsing snapshot: {e}");
        std::process::exit(1);
    });
    println!("Initializing snapshot...");
    HeapSnapshot::new(raw)
}

fn main() {
    let cli = Cli::try_parse().unwrap_or_else(|e| {
        // If no subcommand given, try inserting "view" as default
        let args: Vec<String> = std::env::args().collect();
        if args.len() > 1
            && !args[1].starts_with('-')
            && ![
                "view",
                "summary",
                "retainers",
                "containment",
                "contexts",
                "stack",
                "unreachable",
                "diff",
                "help",
            ]
            .contains(&args[1].as_str())
        {
            if !std::path::Path::new(&args[1]).exists() {
                eprintln!(
                    "error: '{}' is not a recognized subcommand or existing file",
                    args[1]
                );
                std::process::exit(1);
            }
            let mut patched = vec![args[0].clone(), "view".to_string()];
            patched.extend_from_slice(&args[1..]);
            Cli::try_parse_from(&patched).unwrap_or_else(|_| e.exit())
        } else {
            e.exit()
        }
    });

    match cli.command.unwrap() {
        Command::View { main, compare } => {
            let snap = load_snapshot(&main);
            let compare_snaps: Vec<(String, HeapSnapshot)> = compare
                .into_iter()
                .map(|f| {
                    let s = load_snapshot(&f);
                    (f, s)
                })
                .collect();
            tui::run(&main, snap, compare_snaps).unwrap_or_else(|e| {
                eprintln!("Error in interactive mode: {e}");
                std::process::exit(1);
            });
        }
        Command::Summary {
            file,
            depth,
            expand_group,
            expand,
        } => {
            let snap = load_snapshot(&file);
            let expand_ctors = parse_expand_group(&expand_group);
            let expand_ids = parse_expand(&expand);
            print::summary::print_summary(&snap, depth, &expand_ctors, &expand_ids);
        }
        Command::Retainers {
            file,
            object_id,
            depth,
            column_length,
            max_expand_depth,
            max_expand_nodes,
            expand,
        } => {
            let snap = load_snapshot(&file);
            let id = parse_object_id(&object_id).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            let expand = parse_expand(&expand);
            print::retainers::print_retainers(
                &snap,
                id,
                depth,
                &expand,
                column_length,
                print::retainers::RetainerAutoExpandLimits {
                    max_depth: max_expand_depth,
                    max_nodes: max_expand_nodes,
                },
            );
        }
        Command::Containment {
            file,
            object_id,
            depth,
            expand,
        } => {
            let snap = load_snapshot(&file);
            let node_ordinal = match object_id {
                Some(id_str) => {
                    let id = parse_object_id(&id_str).unwrap_or_else(|e| {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    });
                    match snap.node_for_snapshot_object_id(id) {
                        Some(o) => o,
                        None => {
                            eprintln!("Error: no node found with id @{id}");
                            std::process::exit(1);
                        }
                    }
                }
                None => snap.synthetic_root_ordinal(),
            };
            let expand = parse_expand(&expand);
            print::containment::print_containment(&snap, node_ordinal, depth, &expand);
        }
        Command::Contexts { file } => {
            let snap = load_snapshot(&file);
            use types::NodeOrdinal;
            let contexts: Vec<_> = snap
                .native_contexts()
                .iter()
                .map(|&ord| {
                    let ord = NodeOrdinal(ord);
                    let label = snap.native_context_label(ord);
                    let det = match snap.native_context_detachedness(ord) {
                        1 => "no",
                        2 => "yes",
                        _ => "?",
                    };
                    let shallow = snap.node_self_size(ord) as f64;
                    let retained = snap.node_retained_size(ord);
                    let reachable = snap.reachable_size(&[ord]).size;
                    let vars = snap.native_context_vars(ord);
                    (label, det, shallow, retained, reachable, vars)
                })
                .collect();
            let max_label = contexts.iter().map(|(l, ..)| l.len()).max().unwrap_or(0);
            println!(
                "{:<max_label$}  {:>3}  {:>14}  {:>14}  {:>14}",
                "Context", "Det", "Shallow Size", "Retained Size", "Reachable Size"
            );
            println!("{}", "-".repeat(max_label + 54));
            for (label, det, shallow, retained, reachable, vars) in &contexts {
                println!(
                    "{:<max_label$}  {:>3}  {:>14}  {:>14}  {:>14}",
                    label,
                    det,
                    print::format_size(*shallow),
                    print::format_size(*retained),
                    print::format_size(*reachable),
                );
                if !vars.is_empty() {
                    println!("{:max_label$}  Vars: {vars}", "");
                }
            }
        }
        Command::Stack {
            file,
            minimum_reachable_size,
        } => {
            let snap = load_snapshot(&file);
            let min_bytes = minimum_reachable_size.unwrap_or(0.0) * 1024.0 * 1024.0;
            let gc_roots = snap.gc_roots_ordinal();
            let synthetic_root = snap.synthetic_root_ordinal();
            // Collect stack root containers from (GC roots) or synthetic root
            let mut stack_containers = Vec::new();
            if let Some(ord) = snap.find_child_by_node_name(gc_roots, snapshot::V8_STACK_ROOTS) {
                stack_containers.push(ord);
            }
            if let Some(ord) = snap
                .find_child_by_node_name(gc_roots, snapshot::CPPGC_STACK_ROOTS)
                .or_else(|| {
                    snap.find_child_by_node_name(synthetic_root, snapshot::CPPGC_STACK_ROOTS)
                })
            {
                stack_containers.push(ord);
            }
            use types::NodeOrdinal;
            struct StackEntry {
                label: String,
                source: String,
                det: u8,
                retained: f64,
                reachable: f64,
                contexts: Vec<NodeOrdinal>,
            }
            let mut entries: Vec<StackEntry> = Vec::new();
            for container in &stack_containers {
                let source_name = snap.node_raw_name(*container);
                for (_ei, obj_ord) in snap.get_edges(*container) {
                    let name = snap.node_display_name(obj_ord);
                    let node_id = snap.node_id(obj_ord);
                    let det = snap.node_detachedness(obj_ord);
                    let retained = snap.node_retained_size(obj_ord);
                    let info = snap.reachable_size(&[obj_ord]);
                    let label = format!("{name} @{node_id}");
                    entries.push(StackEntry {
                        label,
                        source: source_name.to_string(),
                        det,
                        retained,
                        reachable: info.size,
                        contexts: info.native_contexts,
                    });
                }
            }
            // Sort by reachable size descending
            entries.sort_by(|a, b| b.reachable.partial_cmp(&a.reachable).unwrap());
            entries.retain(|e| e.reachable >= min_bytes);

            let max_label = entries
                .iter()
                .map(|e| e.label.len())
                .max()
                .unwrap_or(0)
                .max(6);
            let max_source = entries
                .iter()
                .map(|e| e.source.len())
                .max()
                .unwrap_or(0)
                .max(6);
            println!(
                "{:<max_label$}  {:<max_source$}  {:>3}  {:>14}  {:>14}",
                "Object", "Source", "Det", "Retained Size", "Reachable Size"
            );
            println!("{}", "-".repeat(max_label + max_source + 38));
            for entry in &entries {
                let det_str = match entry.det {
                    1 => "no",
                    2 => "yes",
                    _ => "?",
                };
                println!(
                    "{:<max_label$}  {:<max_source$}  {:>3}  {:>14}  {:>14}",
                    entry.label,
                    entry.source,
                    det_str,
                    print::format_size(entry.retained),
                    print::format_size(entry.reachable),
                );
                if !entry.contexts.is_empty() {
                    let ctx_strs: Vec<String> = entry
                        .contexts
                        .iter()
                        .map(|&ctx| {
                            let label = snap.native_context_label(ctx);
                            if snap.native_context_detachedness(ctx) == 2 {
                                format!("{label} (Detached)")
                            } else {
                                label
                            }
                        })
                        .collect();
                    println!("    \u{2192} {}", ctx_strs.join(", "));
                }
            }
            println!("\n{} stack-rooted objects", entries.len());
        }
        Command::Unreachable { file, full } => {
            let snap = load_snapshot(&file);
            print::unreachable::print_unreachable(&snap, full);
        }
        Command::Diff {
            main,
            compare,
            expand_group,
            expand: _,
        } => {
            let snap1 = load_snapshot(&main);
            let snap2 = load_snapshot(&compare);
            let expand_groups = parse_expand_group(&expand_group);
            print::diff::print_diff(&snap1, &snap2, &expand_groups);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_only() {
        assert_eq!(split_name_window("Object"), ("Object", None, None));
    }

    #[test]
    fn test_name_with_start() {
        assert_eq!(split_name_window("Object:5"), ("Object", Some(5), None));
    }

    #[test]
    fn test_name_with_start_and_count() {
        assert_eq!(
            split_name_window("Object:5:10"),
            ("Object", Some(5), Some(10))
        );
    }

    #[test]
    fn test_numeric_name() {
        // A bare number is treated as a name with no window
        assert_eq!(split_name_window("123"), ("123", None, None));
    }

    #[test]
    fn test_numeric_name_with_start() {
        assert_eq!(split_name_window("123:5"), ("123", Some(5), None));
    }

    #[test]
    fn test_numeric_name_with_start_and_count() {
        assert_eq!(split_name_window("123:5:10"), ("123", Some(5), Some(10)));
    }

    #[test]
    fn test_name_with_colons() {
        // Name containing colons followed by numeric suffix
        assert_eq!(split_name_window("foo:bar:3"), ("foo:bar", Some(3), None));
    }

    #[test]
    fn test_name_with_colons_and_window() {
        assert_eq!(
            split_name_window("foo:bar:3:10"),
            ("foo:bar", Some(3), Some(10))
        );
    }

    #[test]
    fn test_non_numeric_suffix() {
        assert_eq!(split_name_window("foo:bar"), ("foo:bar", None, None));
    }

    #[test]
    fn test_mixed_non_numeric_suffix() {
        // Only last segment is numeric
        assert_eq!(
            split_name_window("foo:bar:baz"),
            ("foo:bar:baz", None, None)
        );
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(split_name_window(""), ("", None, None));
    }

    #[test]
    fn test_parenthesized_name() {
        assert_eq!(
            split_name_window("(string):0:50"),
            ("(string)", Some(0), Some(50))
        );
    }
}
