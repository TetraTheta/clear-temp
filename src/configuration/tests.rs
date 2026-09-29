use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn parses_rules_and_reports_invalid_lines() {
  let parsed = parse_config("# comment\n%TEMP%\\cache|*.tmp;!keep.tmp|RECURSE\n%TEMP%|*|EMPTY\nmissing-pattern\npath||NONE\n");

  assert_eq!(parsed.rules.len(), 2);
  assert_eq!(parsed.rules[0].option, RemoveOption::Recurse);
  assert_eq!(parsed.rules[0].patterns.positives, ["*.tmp"]);
  assert_eq!(parsed.rules[0].patterns.negatives, ["keep.tmp"]);
  assert_eq!(parsed.rules[1].option, RemoveOption::Empty);
  assert_eq!(parsed.diagnostics.len(), 2);
}

#[test]
fn rejects_partial_empty_rules() {
  let parsed = parse_config("%TEMP%|*.tmp|EMPTY\n%TEMP%|*;!keep.tmp|EMPTY\n");

  assert!(parsed.rules.is_empty());
  assert_eq!(parsed.diagnostics.len(), 2);
}

#[test]
fn expands_variables_case_insensitively_and_rejects_unknown_names() {
  let expanded = expand_path("%lOcAlApPdAtA%\\ClearTemp").unwrap();
  assert_eq!(expanded.len(), 1);
  assert!(expanded[0].ends_with("ClearTemp"));
  assert!(expand_path("%CLEARTEMP_MISSING_VARIABLE%\\x").is_err());
}

#[test]
fn finds_mixed_case_config_name() {
  let directory = temporary_directory("config-name");
  fs::write(directory.join("cLEARtEMP.txt"), b"%TEMP%|*").unwrap();

  let found = find_config_file(&directory, &directory).unwrap();
  assert_eq!(found.file_name().unwrap(), "cLEARtEMP.txt");

  fs::remove_dir_all(directory).unwrap();
}

#[test]
fn loads_utf8_bom_and_expands_both_program_files_locations() {
  let directory = temporary_directory("utf8-bom");
  let config = directory.join("ClearTemp.txt");
  fs::write(&config, b"\xEF\xBB\xBF%TEMP%|*\n").unwrap();

  let parsed = load_config(&config).unwrap();
  assert_eq!(parsed.rules.len(), 1);

  let locations = expand_path(r"%ProgramFiles%\ClearTemp").unwrap();
  assert_eq!(locations.len(), 2);
  assert_ne!(path_key(&locations[0]), path_key(&locations[1]));

  fs::remove_dir_all(directory).unwrap();
}

#[test]
fn accepts_an_explicit_config_path_and_rejects_an_empty_path() {
  let selected = select_config_file(Some(OsString::from("custom rules.txt"))).unwrap();
  assert_eq!(selected, PathBuf::from("custom rules.txt"));

  let error = select_config_file(Some(OsString::new())).unwrap_err();
  assert_eq!(error, "Configuration file path must not be empty.");
}

fn temporary_directory(label: &str) -> PathBuf {
  let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
  let path = env::temp_dir().join(format!("cleartemp-{label}-{}-{unique}", std::process::id()));
  fs::create_dir(&path).unwrap();
  path
}
