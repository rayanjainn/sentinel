//! Tool specifications sent to every provider. Schemas use only the subset every provider accepts
//! verbatim: `type`, `properties`, `required`, `items`, `enum`, `description`, `minimum`,
//! `maximum` (no `$ref`, `oneOf`, `anyOf`, defaults or formats).

use serde_json::{Value, json};

use super::{ToolAccess, ToolSpec, names};
use crate::backend::ToolDefinition;

const REASON: &str = "One or two sentences for the user explaining why this change is proposed, citing the live numbers you observed (size, CPU %, last-used date, port).";

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": properties, "required": required})
}

fn read(name: &'static str, description: &'static str, input_schema: Value) -> ToolSpec {
    ToolSpec {
        name,
        description,
        access: ToolAccess::Read,
        input_schema,
    }
}

fn write(name: &'static str, description: &'static str, input_schema: Value) -> ToolSpec {
    ToolSpec {
        name,
        description,
        access: ToolAccess::Write,
        input_schema,
    }
}

fn path_arg(what: &str) -> Value {
    json!({"type": "string", "description": format!("Absolute path {what}. `~` expands to the user's home directory.")})
}

pub fn all() -> Vec<ToolSpec> {
    vec![
        read(
            names::GET_SYSTEM_OVERVIEW,
            "Snapshot of this machine right now: OS, CPU model and core count, total CPU %, memory and swap use, load average, every mounted volume with free space, permission status, process count and the top processes by CPU. Call this first when the request is broad.",
            object(json!({}), &[]),
        ),
        read(
            names::GET_RESOURCE_USAGE,
            "Live CPU (total and per core), memory breakdown (used, cached, compressed, swap), load average, temperatures/fans when the OS exposes them, and network throughput. Optionally summarizes the last N seconds (average and peak) to tell a spike from sustained load.",
            object(
                json!({
                    "history_seconds": {"type": "integer", "minimum": 0, "maximum": 900, "description": "Also summarize this many seconds of recent history (0 = current sample only)."}
                }),
                &[],
            ),
        ),
        read(
            names::LIST_PROCESSES,
            "Running processes with PID, parent PID, name, user, CPU % (100 = one full core, instant and rolling average), resident memory, status and run time. Sorted and truncated; the result reports the total count.",
            object(
                json!({
                    "sort_by": {"type": "string", "enum": ["cpu", "memory", "name", "pid", "start_time"], "description": "Sort key, descending for cpu/memory/start_time. Default cpu."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100, "description": "Maximum rows. Default 25."},
                    "name_contains": {"type": "string", "description": "Case-insensitive filter on process name or executable path."},
                    "user": {"type": "string", "description": "Only processes owned by this user."}
                }),
                &[],
            ),
        ),
        read(
            names::GET_PROCESS_DETAILS,
            "Full details for one PID: command line, executable, user, start time, CPU and memory, threads, open file count with a sample of open files, network connections, and whether graceful termination will close a window. Use before proposing any action on a process.",
            object(
                json!({"pid": {"type": "integer", "minimum": 0, "description": "Process ID."}}),
                &["pid"],
            ),
        ),
        read(
            names::LIST_NETWORK_SOCKETS,
            "Open TCP/UDP sockets with local and remote address, state, owning process (name and PID), reverse-resolved host, and traffic where available.",
            object(
                json!({
                    "state": {"type": "string", "enum": ["all", "listen", "established"], "description": "Filter by TCP state. Default all."},
                    "protocol": {"type": "string", "enum": ["tcp", "udp"], "description": "Only this protocol."},
                    "pid": {"type": "integer", "minimum": 0, "description": "Only sockets owned by this PID."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 200, "description": "Maximum rows. Default 50."}
                }),
                &[],
            ),
        ),
        read(
            names::FIND_PORT_OWNER,
            "Which process owns a local port: every socket bound to it (listeners first) with the owning process's PID, name, user, executable and start time.",
            object(
                json!({
                    "port": {"type": "integer", "minimum": 1, "maximum": 65535, "description": "Local port number."},
                    "protocol": {"type": "string", "enum": ["tcp", "udp"], "description": "Only this protocol. Default both."}
                }),
                &["port"],
            ),
        ),
        read(
            names::LIST_VOLUMES,
            "Mounted volumes with mount point, file system, total and free bytes, and percent used.",
            object(json!({}), &[]),
        ),
        read(
            names::SCAN_STORAGE,
            "Scans a directory tree (reusing a recent completed scan that covers it) and returns totals, unreadable entries, the largest files, space by file type, and cleanup suggestion counts. Large trees can take minutes.",
            object(
                json!({
                    "path": path_arg("to scan, e.g. ~ or /"),
                    "force_rescan": {"type": "boolean", "description": "Ignore existing scans and walk again. Default false."}
                }),
                &["path"],
            ),
        ),
        read(
            names::GET_STORAGE_BREAKDOWN,
            "Size of each child folder and file under a path (largest first), with item counts and last modified/accessed dates. Scans first if needed.",
            object(
                json!({
                    "path": path_arg("of the folder to break down"),
                    "depth": {"type": "integer", "minimum": 1, "maximum": 3, "description": "Levels of children to include. Default 1."},
                    "max_children": {"type": "integer", "minimum": 1, "maximum": 50, "description": "Children per folder. Default 15."}
                }),
                &["path"],
            ),
        ),
        read(
            names::FIND_LARGE_FILES,
            "Largest files under a path, optionally only those not used (neither modified nor accessed) for a number of days. Reports size, last modified, last accessed and days since last use. Scans first if needed.",
            object(
                json!({
                    "path": path_arg("to search under"),
                    "min_size_mb": {"type": "number", "minimum": 0, "description": "Only files at least this large, in MB. Default 100."},
                    "unused_for_days": {"type": "integer", "minimum": 1, "maximum": 36500, "description": "Only files whose last modification AND last access are older than this many days."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 200, "description": "Maximum files. Default 50."}
                }),
                &["path"],
            ),
        ),
        read(
            names::GET_CLEANUP_SUGGESTIONS,
            "Space commonly safe to reclaim under a path: app caches, logs, old downloads, build artifacts (node_modules, target), package manager and developer caches, each with size, last used date and why it is usually safe. Suggestions only; Trash contents are informational.",
            object(
                json!({
                    "path": path_arg("to look under. Default the home directory"),
                    "unused_for_days": {"type": "integer", "minimum": 1, "maximum": 36500, "description": "Only suggestions not modified or accessed for this many days."}
                }),
                &[],
            ),
        ),
        read(
            names::FIND_DUPLICATE_FILES,
            "Groups of byte-identical files under a path (content hash), with reclaimable bytes per group. Hashing can take a while on large trees.",
            object(
                json!({
                    "path": path_arg("to search under"),
                    "min_size_mb": {"type": "number", "minimum": 0, "description": "Ignore files smaller than this, in MB. Default 1."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100, "description": "Maximum groups, largest reclaimable first. Default 20."}
                }),
                &["path"],
            ),
        ),
        read(
            names::LIST_FIREWALL_RULES,
            "Firewall backend status and the rules Sentinel created (id, blocked IP or port, direction, whether currently active). The user's other firewall rules are never listed or changed.",
            object(json!({}), &[]),
        ),
        write(
            names::TERMINATE_PROCESS,
            "Proposes gracefully stopping a process (SIGTERM, or a quit request for apps with windows). Does NOT run it: the action is queued as a plan card the user must approve. Call get_process_details or list_processes first in this turn.",
            object(
                json!({
                    "pid": {"type": "integer", "minimum": 2, "description": "Process ID observed in this turn."},
                    "reason": {"type": "string", "description": REASON}
                }),
                &["pid", "reason"],
            ),
        ),
        write(
            names::FORCE_KILL_PROCESS,
            "Proposes force killing a process (SIGKILL / TerminateProcess; unsaved work is lost). Prefer terminate_process unless the process is hung. Queued for user approval, never run directly.",
            object(
                json!({
                    "pid": {"type": "integer", "minimum": 2, "description": "Process ID observed in this turn."},
                    "reason": {"type": "string", "description": REASON}
                }),
                &["pid", "reason"],
            ),
        ),
        write(
            names::SET_PROCESS_PRIORITY,
            "Proposes changing a process's scheduling priority on the Unix nice scale (-20 highest to 19 lowest; raising priority needs administrator rights). Queued for user approval.",
            object(
                json!({
                    "pid": {"type": "integer", "minimum": 2, "description": "Process ID observed in this turn."},
                    "nice": {"type": "integer", "minimum": -20, "maximum": 19, "description": "Target nice value. 10 is a good 'background' value."},
                    "reason": {"type": "string", "description": REASON}
                }),
                &["pid", "nice", "reason"],
            ),
        ),
        write(
            names::MOVE_TO_TRASH,
            "Proposes moving files or folders to the OS Trash / Recycle Bin (recoverable; nothing is ever permanently deleted). Use exact paths returned by storage tools in this turn. Queued for user approval.",
            object(
                json!({
                    "paths": {"type": "array", "items": {"type": "string"}, "description": "Absolute paths to move to the Trash (1 to 500)."},
                    "reason": {"type": "string", "description": REASON}
                }),
                &["paths", "reason"],
            ),
        ),
        write(
            names::MOVE_PATHS,
            "Proposes moving files or folders into an existing destination folder (for example an external drive). Queued for user approval.",
            object(
                json!({
                    "paths": {"type": "array", "items": {"type": "string"}, "description": "Absolute paths to move (1 to 500)."},
                    "destination_dir": path_arg("of an existing destination folder"),
                    "reason": {"type": "string", "description": REASON}
                }),
                &["paths", "destination_dir", "reason"],
            ),
        ),
        write(
            names::BLOCK_REMOTE_IP,
            "Proposes a firewall rule blocking traffic to and/or from a remote IP address (requires administrator approval). Queued for user approval.",
            object(
                json!({
                    "ip": {"type": "string", "description": "IPv4 or IPv6 address observed in this turn."},
                    "direction": {"type": "string", "enum": ["inbound", "outbound", "both"], "description": "Traffic to block. Default both."},
                    "reason": {"type": "string", "description": REASON}
                }),
                &["ip", "reason"],
            ),
        ),
        write(
            names::BLOCK_LOCAL_PORT,
            "Proposes a firewall rule blocking a local port (requires administrator approval). To free a port, prefer terminating its owner. Queued for user approval.",
            object(
                json!({
                    "port": {"type": "integer", "minimum": 1, "maximum": 65535, "description": "Local port number."},
                    "protocol": {"type": "string", "enum": ["tcp", "udp"], "description": "Transport protocol."},
                    "direction": {"type": "string", "enum": ["inbound", "outbound", "both"], "description": "Traffic to block. Default inbound."},
                    "reason": {"type": "string", "description": REASON}
                }),
                &["port", "protocol", "reason"],
            ),
        ),
        write(
            names::REMOVE_FIREWALL_RULE,
            "Proposes removing a firewall rule Sentinel created, by id from list_firewall_rules. Queued for user approval.",
            object(
                json!({
                    "rule_id": {"type": "string", "description": "Rule id from list_firewall_rules."},
                    "reason": {"type": "string", "description": REASON}
                }),
                &["rule_id", "reason"],
            ),
        ),
    ]
}

pub fn spec(name: &str) -> Option<ToolSpec> {
    all().into_iter().find(|s| s.name == name)
}

pub fn definitions() -> Vec<ToolDefinition> {
    all()
        .into_iter()
        .map(|s| ToolDefinition {
            name: s.name.to_owned(),
            description: s.description.to_owned(),
            input_schema: s.input_schema,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::{TOOL_ACCESS, access_of};
    use super::*;

    const ALLOWED: &[&str] = &[
        "type",
        "properties",
        "required",
        "items",
        "enum",
        "description",
        "minimum",
        "maximum",
    ];

    fn check_schema(tool: &str, schema: &Value, at_property_map: bool) {
        let object = schema
            .as_object()
            .unwrap_or_else(|| panic!("{tool}: schema must be an object"));
        for (key, value) in object {
            if at_property_map {
                check_schema(tool, value, false);
                continue;
            }
            assert!(
                ALLOWED.contains(&key.as_str()),
                "{tool}: keyword `{key}` is not provider-safe"
            );
            match key.as_str() {
                "properties" => check_schema(tool, value, true),
                "items" => check_schema(tool, value, false),
                _ => {}
            }
        }
        if !at_property_map && object.get("type") == Some(&json!("object")) {
            let props = object["properties"]
                .as_object()
                .expect("object schemas list properties");
            for required in object["required"].as_array().expect("required array") {
                assert!(
                    props.contains_key(required.as_str().unwrap()),
                    "{tool}: required `{required}` undefined"
                );
            }
        }
    }

    #[test]
    fn every_registered_tool_has_one_spec_with_matching_access() {
        let specs = all();
        assert_eq!(specs.len(), TOOL_ACCESS.len());
        for (name, access) in TOOL_ACCESS {
            let matching: Vec<_> = specs.iter().filter(|s| s.name == *name).collect();
            assert_eq!(matching.len(), 1, "{name}");
            assert_eq!(matching[0].access, *access, "{name}");
        }
        for spec in &specs {
            assert_eq!(access_of(spec.name), Some(spec.access));
        }
    }

    #[test]
    fn schemas_use_only_the_portable_subset() {
        for spec in all() {
            assert_eq!(spec.input_schema["type"], "object", "{}", spec.name);
            check_schema(spec.name, &spec.input_schema, false);
            assert!(
                spec.description.len() > 40,
                "{} needs a precise description",
                spec.name
            );
        }
    }

    #[test]
    fn write_tools_require_a_reason_and_say_they_are_queued() {
        for spec in all().into_iter().filter(|s| s.access == ToolAccess::Write) {
            let required = spec.input_schema["required"].as_array().unwrap();
            assert!(required.contains(&json!("reason")), "{}", spec.name);
            assert!(spec.description.contains("approv"), "{}", spec.name);
        }
    }
}
