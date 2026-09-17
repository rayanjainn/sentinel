//! Interpreters: name the script, module or jar being run, not just "node".

use super::Draft;
use super::process::{ProcessFacts, file_name, stem};
use crate::model::{Confidence, ProcessCategory, ProcessRole, QuitSafety};

struct Language {
    /// Plain name of the runtime.
    label: &'static str,
    /// What the thing it runs is called.
    unit: &'static str,
}

fn language(binary: &str) -> Option<Language> {
    let lower = binary.to_ascii_lowercase();
    let lower = stem(&lower);
    let is_python = lower == "python"
        || lower == "pythonw"
        || lower
            .strip_prefix("python")
            .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit() || c == '.'));
    if is_python {
        return Some(Language {
            label: "Python",
            unit: "script",
        });
    }
    match lower {
        "node" | "nodejs" => Some(Language {
            label: "Node.js",
            unit: "script",
        }),
        "deno" => Some(Language {
            label: "Deno",
            unit: "script",
        }),
        "bun" => Some(Language {
            label: "Bun",
            unit: "script",
        }),
        "ruby" => Some(Language {
            label: "Ruby",
            unit: "script",
        }),
        "java" => Some(Language {
            label: "Java",
            unit: "program",
        }),
        _ => None,
    }
}

pub(super) fn classify(facts: &ProcessFacts<'_>) -> Option<Draft> {
    let binary = file_name(facts.exe).unwrap_or(facts.name);
    let language = language(binary)?;
    let entry = entry_point(facts);
    let started_by = facts
        .parent
        .filter(|parent| parent.pid > 1)
        .map(|parent| parent.name);

    let (headline, mut detail) = match &entry {
        Some(entry) => (
            format!("{entry} — {} {}", language.label, language.unit),
            format!(
                "A {} {} called {entry}, run by {binary}. Sentinel reads the name from the command \
                 line; what the {} actually does is up to that file.",
                language.label, language.unit, language.unit
            ),
        ),
        None => (
            format!("{} — no {} named", language.label, language.unit),
            format!(
                "The {} runtime is running, but its command line does not name a {}, so Sentinel \
                 cannot say what it is running.",
                language.label, language.unit
            ),
        ),
    };
    if let Some(parent) = started_by {
        detail.push_str(&format!(" Started by {parent}."));
    }
    Some(
        Draft::new(
            headline,
            detail,
            ProcessRole::Interpreter,
            ProcessCategory::Developer,
            QuitSafety::Background,
        )
        .confidence(if entry.is_some() {
            Confidence::Known
        } else {
            Confidence::Unknown
        }),
    )
}

/// The script, module or jar the interpreter was pointed at.
fn entry_point(facts: &ProcessFacts<'_>) -> Option<String> {
    let mut args = facts.cmd.iter().map(String::as_str).skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg {
            // `python -m http.server` runs a module by name.
            "-m" | "--module" => return args.next().map(str::to_owned),
            // `java -jar app.jar`.
            "-jar" => {
                return args
                    .next()
                    .map(|jar| file_name(Some(jar)).unwrap_or(jar).to_owned());
            }
            "-c" | "-e" | "--eval" | "--print" => {
                return Some("inline code".to_owned());
            }
            // Flags that take a separate value, whose value is not the entry point.
            "--add-modules" | "--add-opens" | "--classpath" | "-cp" | "-classpath"
            | "--require" | "-r" | "--import" => {
                args.next();
            }
            _ if arg.starts_with('-') => {}
            _ => {
                let name = file_name(Some(arg)).unwrap_or(arg);
                return (!name.is_empty()).then(|| name.to_owned());
            }
        }
    }
    None
}
