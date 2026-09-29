#![cfg_attr(not(windows), allow(dead_code, unused_imports))]

#[cfg(not(windows))]
compile_error!("ClearTemp supports Windows only.");

mod configuration;
mod console;
mod io;

use std::collections::HashSet;
use std::path::Path;
use std::process::ExitCode;

use configuration::{PatternSet, RemoveOption, expand_path, load_config, path_key, select_config_file};
use console::{print_error_message, print_info, print_warning, wait_for_key};
use io::{expand_directories, process};

fn main() -> ExitCode {
  let result = run();
  wait_for_key();
  result
}

fn run() -> ExitCode {
  let config_path = match select_config_file(std::env::args_os().skip(1)) {
    Ok(path) => path,
    Err(error) => {
      print_error_message(&error);
      return ExitCode::FAILURE;
    },
  };
  let parsed = match load_config(&config_path) {
    Ok(parsed) => parsed,
    Err(error) => {
      print_error_message(&error);
      return ExitCode::FAILURE;
    },
  };

  for diagnostic in &parsed.diagnostics {
    print_warning(&format!("Line {}: {}", diagnostic.line, diagnostic.message));
  }

  let mut seen = HashSet::new();
  let mut cleanups = Vec::new();
  for rule in &parsed.rules {
    let path_patterns = match expand_path(&rule.path) {
      Ok(paths) => paths,
      Err(error) => {
        print_warning(&format!("Line {}: {error}", rule.line));
        continue;
      },
    };

    for directory in expand_directories(&path_patterns) {
      if is_new_cleanup(&mut seen, &directory, &rule.patterns, rule.option) {
        cleanups.push((directory, &rule.patterns, rule.option));
      }
    }
  }

  for (directory, patterns, option) in cleanups {
    println!();
    print_info(&directory);
    process(&directory, patterns, option);
  }

  ExitCode::SUCCESS
}

fn is_new_cleanup<'a>(
  seen: &mut HashSet<(String, &'a PatternSet, RemoveOption)>,
  directory: &Path,
  patterns: &'a PatternSet,
  option: RemoveOption,
) -> bool {
  seen.insert((path_key(directory), patterns, option))
}

#[cfg(test)]
mod tests;
