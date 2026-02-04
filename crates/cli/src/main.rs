mod args;
mod json;
mod path;
mod text;

use std::env;
use std::io;
use std::path::PathBuf;

use crate::args::{Localization, ParsedArgs, detect_localization, parse_args, print_help, tr};
use crate::json::{JsonScanStats, map_duplicate_groups, map_report, map_span_groups, write_json};
use crate::path::resolve_path;
use crate::text::{
    format_fatal_skip_warning, format_scan_stats, format_text, format_text_code_spans,
    format_text_report, has_fatal_skips,
};

fn args_before_dashdash(args: &[String]) -> &[String] {
    match args.iter().position(|a| a == "--") {
        Some(pos) => &args[..pos],
        None => args,
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let pre_dashdash = args_before_dashdash(&args);
    if pre_dashdash.iter().any(|a| a == "-V" || a == "--version") {
        println!("dup-code-check {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let localization = match detect_localization(&args) {
        Ok(localization) => localization,
        Err(message) => {
            eprintln!("Error: {message}\n");
            print_help(Localization::En);
            std::process::exit(2);
        }
    };

    if pre_dashdash.iter().any(|a| a == "-h" || a == "--help") {
        print_help(localization);
        return;
    }

    let parsed = match parse_args(&args, localization) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{}: {message}\n", tr(localization, "Error", "错误"),);
            print_help(localization);
            std::process::exit(2);
        }
    };

    let roots: Vec<PathBuf> = match parsed
        .roots
        .iter()
        .map(|p| resolve_path(p))
        .collect::<io::Result<Vec<_>>>()
    {
        Ok(v) => v,
        Err(err) => {
            eprintln!("{}: {err}", tr(localization, "Error", "错误"));
            std::process::exit(1);
        }
    };

    match run(&parsed, &roots) {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(err) => {
            eprintln!("{}: {err}", tr(localization, "Error", "错误"));
            std::process::exit(1);
        }
    }
}

fn run(parsed: &ParsedArgs, roots: &[PathBuf]) -> io::Result<i32> {
    if parsed.report {
        let outcome =
            dup_code_check_core::generate_duplication_report_with_stats(roots, &parsed.options)?;
        let report = map_report(outcome.result);
        let scan_stats = outcome.stats;

        if parsed.json {
            if parsed.stats {
                write_json(&serde_json::json!({
                    "report": report,
                    "scanStats": Some(JsonScanStats::from(&scan_stats)),
                }))?;
            } else {
                write_json(&report)?;
            }
        } else {
            print!("{}", format_text_report(parsed.localization, &report));
        }
        return finalize_scan(parsed, &scan_stats);
    }

    if parsed.code_spans {
        let outcome =
            dup_code_check_core::find_duplicate_code_spans_with_stats(roots, &parsed.options)?;
        let groups = map_span_groups(outcome.result);
        let scan_stats = outcome.stats;

        if parsed.json {
            if parsed.stats {
                write_json(&serde_json::json!({
                    "groups": groups,
                    "scanStats": Some(JsonScanStats::from(&scan_stats)),
                }))?;
            } else {
                write_json(&groups)?;
            }
        } else {
            print!("{}", format_text_code_spans(parsed.localization, &groups));
        }
        return finalize_scan(parsed, &scan_stats);
    }

    let outcome = dup_code_check_core::find_duplicate_files_with_stats(roots, &parsed.options)?;
    let groups = map_duplicate_groups(outcome.result);
    let scan_stats = outcome.stats;

    if parsed.json {
        if parsed.stats {
            write_json(&serde_json::json!({
                "groups": groups,
                "scanStats": Some(JsonScanStats::from(&scan_stats)),
            }))?;
        } else {
            write_json(&groups)?;
        }
    } else {
        print!("{}", format_text(parsed.localization, &groups));
    }

    finalize_scan(parsed, &scan_stats)
}

fn finalize_scan(
    parsed: &ParsedArgs,
    scan_stats: &dup_code_check_core::ScanStats,
) -> io::Result<i32> {
    if parsed.stats && !parsed.json {
        eprint!("{}", format_scan_stats(parsed.localization, scan_stats));
    }

    if has_fatal_skips(scan_stats) {
        eprint!(
            "{}",
            format_fatal_skip_warning(
                parsed.localization,
                scan_stats,
                parsed.stats || parsed.strict
            )
        );
    }

    if parsed.strict && has_fatal_skips(scan_stats) {
        if !parsed.stats {
            eprint!("{}", format_scan_stats(parsed.localization, scan_stats));
        }
        return Ok(1);
    }

    Ok(0)
}
