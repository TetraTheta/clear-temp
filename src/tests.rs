use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::configuration::{expand_path, parse_config};
use crate::io::expand_directories;

#[test]
fn parses_expands_and_resolves_a_rule() {
  let root = temporary_directory();
  let target = root.join("Chrome Stable").join("User Data").join("Default");
  fs::create_dir_all(&target).unwrap();
  let config = format!(r"{}\Chrome*\User Data\*|*Web Data", root.display());

  let parsed = parse_config(&config);
  assert!(parsed.diagnostics.is_empty());
  let patterns = expand_path(&parsed.rules[0].path).unwrap();
  assert_eq!(expand_directories(&patterns), [target]);

  fs::remove_dir_all(root).unwrap();
}

#[test]
fn keeps_distinct_rules_and_removes_only_exact_duplicates() {
  let parsed = parse_config(
    r"C:\Users\Example\AppData\Local\Temp|*|EMPTY
c:\users\example\appdata\local\temp\|*|EMPTY
C:\Users\Example\AppData\Local\Temp|*.tmp|NONE",
  );
  let mut seen = HashSet::new();

  assert!(super::is_new_cleanup(&mut seen, Path::new(&parsed.rules[0].path), &parsed.rules[0].patterns, parsed.rules[0].option,));
  assert!(!super::is_new_cleanup(&mut seen, Path::new(&parsed.rules[1].path), &parsed.rules[1].patterns, parsed.rules[1].option,));
  assert!(super::is_new_cleanup(&mut seen, Path::new(&parsed.rules[2].path), &parsed.rules[2].patterns, parsed.rules[2].option,));
}

fn temporary_directory() -> std::path::PathBuf {
  let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
  let path = std::env::temp_dir().join(format!("cleartemp-flow-{}-{unique}", std::process::id()));
  fs::create_dir(&path).unwrap();
  path
}
