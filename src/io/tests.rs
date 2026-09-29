use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn expands_nested_wildcards_and_deduplicates_paths() {
  let root = temporary_directory("wildcards");
  let profile = root.join("Google").join("Chrome Stable").join("User Data").join("Default");
  fs::create_dir_all(&profile).unwrap();
  let pattern = root.join("Google").join("Chrome*").join("User Data").join("*");

  let expanded = expand_directories(&[pattern.clone(), pattern]);
  assert_eq!(expanded, [profile]);

  fs::remove_dir_all(root).unwrap();
}

#[test]
fn applies_none_recurse_and_remove_self_semantics() {
  let root = temporary_directory("options");
  let nested = root.join("nested");
  fs::create_dir(&nested).unwrap();
  fs::write(root.join("root.tmp"), b"").unwrap();
  fs::write(nested.join("nested.tmp"), b"").unwrap();
  let patterns = PatternSet { positives: vec!["*.tmp".to_owned()], negatives: Vec::new() };

  process(&root, &patterns, RemoveOption::None);
  assert!(!root.join("root.tmp").exists());
  assert!(nested.join("nested.tmp").exists());

  process(&root, &patterns, RemoveOption::RemoveSelf);
  assert!(!root.exists());
}

#[test]
fn empties_contents_without_removing_target() {
  let root = temporary_directory("empty");
  let nested = root.join("nested").join("deeper");
  fs::create_dir_all(&nested).unwrap();
  fs::write(root.join("root.tmp"), b"").unwrap();
  fs::write(nested.join("nested.tmp"), b"").unwrap();
  let patterns = PatternSet { positives: vec!["*".to_owned()], negatives: Vec::new() };

  process(&root, &patterns, RemoveOption::Empty);

  assert!(root.exists());
  assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
  fs::remove_dir(root).unwrap();
}

#[test]
fn preserves_excluded_files_and_non_empty_directories() {
  let root = temporary_directory("exclude");
  fs::write(root.join("delete.log"), b"").unwrap();
  fs::write(root.join("keep.log"), b"").unwrap();
  let patterns = PatternSet { positives: vec!["*.log".to_owned()], negatives: vec!["keep.log".to_owned()] };

  process(&root, &patterns, RemoveOption::RemoveSelf);
  assert!(!root.join("delete.log").exists());
  assert!(root.join("keep.log").exists());
  assert!(root.exists());

  fs::remove_dir_all(root).unwrap();
}

#[test]
fn converts_drive_and_unc_paths_to_verbatim_form() {
  assert_eq!(to_verbatim(Path::new(r"E:\Temp")), PathBuf::from(r"\\?\E:\Temp"));
  assert_eq!(to_verbatim(Path::new(r"\\server\share\Temp")), PathBuf::from(r"\\?\UNC\server\share\Temp"));
}

fn temporary_directory(label: &str) -> PathBuf {
  let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
  let path = std::env::temp_dir().join(format!("cleartemp-{label}-{}-{unique}", std::process::id()));
  fs::create_dir(&path).unwrap();
  path
}
