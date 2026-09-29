#![cfg_attr(not(windows), allow(dead_code, unused_imports))]

#[cfg(not(windows))]
compile_error!("ClearTemp supports Windows only.");

mod configuration;
mod console;
mod elevation;
mod io;

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::ExitCode;

use configuration::{PatternSet, RemoveOption, expand_path, load_config, path_key, select_config_file};
use console::{print_error_message, print_info, print_warning, wait_for_key};
use elevation::{is_elevated, run_elevated};
use io::{expand_directories, process};

const USAGE: &str = "Usage: ClearTemp.exe [--no-elevate] [--no-wait] [configuration-file]";

#[derive(Debug, Default, Eq, PartialEq)]
struct Arguments {
  config_file: Option<OsString>,
  no_elevate: bool,
  no_wait: bool,
}

fn main() -> ExitCode {
  let raw_arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
  let no_wait =
    raw_arguments.iter().take_while(|argument| argument.as_os_str() != OsStr::new("--")).any(|argument| argument == OsStr::new("--no-wait"));
  let arguments = match parse_arguments(raw_arguments.iter().cloned()) {
    Ok(arguments) => arguments,
    Err(error) => {
      print_error_message(&error);
      if !no_wait {
        wait_for_key();
      }
      return ExitCode::FAILURE;
    },
  };

  if !arguments.no_elevate && !is_elevated() {
    return match run_elevated(&raw_arguments) {
      Ok(result) => result,
      Err(error) => {
        print_error_message(&format!("Cannot request administrator privileges: {error}"));
        if !arguments.no_wait {
          wait_for_key();
        }
        ExitCode::FAILURE
      },
    };
  }

  let result = run(arguments.config_file);
  if !arguments.no_wait {
    wait_for_key();
  }
  result
}

fn run(config_file: Option<OsString>) -> ExitCode {
  let config_path = match select_config_file(config_file) {
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

fn parse_arguments(arguments: impl IntoIterator<Item = OsString>) -> Result<Arguments, String> {
  let mut parsed = Arguments::default();
  let mut parse_options = true;

  for argument in arguments {
    if parse_options && argument == OsStr::new("--no-elevate") {
      parsed.no_elevate = true;
    } else if parse_options && argument == OsStr::new("--no-wait") {
      parsed.no_wait = true;
    } else if parse_options && argument == OsStr::new("--") {
      parse_options = false;
    } else if parse_options && argument.to_string_lossy().starts_with('-') {
      return Err(format!("Unknown option '{}'.\n{USAGE}", argument.to_string_lossy()));
    } else if argument.is_empty() || parsed.config_file.replace(argument).is_some() {
      return Err(USAGE.to_owned());
    }
  }

  Ok(parsed)
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
