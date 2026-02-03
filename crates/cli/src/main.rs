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
    format_scan_stats, format_text, format_text_code_spans, format_text_report, has_fatal_skips,
};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-V" || a == "--version") {
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

    if args.iter().any(|a| a == "-h" || a == "--help") {
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

    let need_stats = parsed.stats || parsed.strict;
    match run(&parsed, &roots, need_stats) {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(err) => {
            eprintln!("{}: {err}", tr(localization, "Error", "错误"));
            std::process::exit(1);
        }
    }
}

fn run(parsed: &ParsedArgs, roots: &[PathBuf], need_stats: bool) -> io::Result<i32> {
    if parsed.report {
        let (report, scan_stats) = if need_stats {
            let outcome = dup_code_check_core::generate_duplication_report_with_stats(
                roots,
                &parsed.options,
            )?;
            (map_report(outcome.result), Some(outcome.stats))
        } else {
            let report = dup_code_check_core::generate_duplication_report(roots, &parsed.options)?;
            (map_report(report), None)
        };

        if parsed.json {
            if parsed.stats {
                write_json(&serde_json::json!({
                    "report": report,
                    "scanStats": scan_stats.clone().map(JsonScanStats::from),
                }))?;
            } else {
                write_json(&report)?;
            }
        } else {
            print!("{}", format_text_report(parsed.localization, &report));
        }

        if let Some(stats) = scan_stats {
            if parsed.stats && !parsed.json {
                eprint!("{}", format_scan_stats(parsed.localization, &stats));
            }
            if parsed.strict && has_fatal_skips(&stats) {
                if !parsed.stats {
                    eprint!("{}", format_scan_stats(parsed.localization, &stats));
                }
                return Ok(1);
            }
        }

        return Ok(0);
    }

    if parsed.code_spans {
        let (groups, scan_stats) = if need_stats {
            let outcome =
                dup_code_check_core::find_duplicate_code_spans_with_stats(roots, &parsed.options)?;
            (map_span_groups(outcome.result), Some(outcome.stats))
        } else {
            let groups = dup_code_check_core::find_duplicate_code_spans(roots, &parsed.options)?;
            (map_span_groups(groups), None)
        };

        if parsed.json {
            if parsed.stats {
                write_json(&serde_json::json!({
                    "groups": groups,
                    "scanStats": scan_stats.clone().map(JsonScanStats::from),
                }))?;
            } else {
                write_json(&groups)?;
            }
        } else {
            print!("{}", format_text_code_spans(parsed.localization, &groups));
        }

        if let Some(stats) = scan_stats {
            if parsed.stats && !parsed.json {
                eprint!("{}", format_scan_stats(parsed.localization, &stats));
            }
            if parsed.strict && has_fatal_skips(&stats) {
                if !parsed.stats {
                    eprint!("{}", format_scan_stats(parsed.localization, &stats));
                }
                return Ok(1);
            }
        }

        return Ok(0);
    }

    let (groups, scan_stats) = if need_stats {
        let outcome = dup_code_check_core::find_duplicate_files_with_stats(roots, &parsed.options)?;
        (map_duplicate_groups(outcome.result), Some(outcome.stats))
    } else {
        let groups = dup_code_check_core::find_duplicate_files(roots, &parsed.options)?;
        (map_duplicate_groups(groups), None)
    };

    if parsed.json {
        if parsed.stats {
            write_json(&serde_json::json!({
                "groups": groups,
                "scanStats": scan_stats.clone().map(JsonScanStats::from),
            }))?;
        } else {
            write_json(&groups)?;
        }
    } else {
        print!("{}", format_text(parsed.localization, &groups));
    }

    if let Some(stats) = scan_stats {
        if parsed.stats && !parsed.json {
            eprint!("{}", format_scan_stats(parsed.localization, &stats));
        }
        if parsed.strict && has_fatal_skips(&stats) {
            if !parsed.stats {
                eprint!("{}", format_scan_stats(parsed.localization, &stats));
            }
            return Ok(1);
        }
    }

    Ok(0)
}
