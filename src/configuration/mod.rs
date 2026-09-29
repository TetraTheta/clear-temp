use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr;

use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::{
  FOLDERID_Documents, FOLDERID_LocalAppData, FOLDERID_LocalAppDataLow, FOLDERID_Music, FOLDERID_Pictures, FOLDERID_Profile, FOLDERID_ProgramData,
  FOLDERID_ProgramFilesCommonX64, FOLDERID_ProgramFilesCommonX86, FOLDERID_ProgramFilesX64, FOLDERID_ProgramFilesX86, FOLDERID_Public,
  FOLDERID_RoamingAppData, FOLDERID_Videos, FOLDERID_Windows, SHGetKnownFolderPath,
};
use windows_sys::core::GUID;

use crate::console::display_path;

const CONFIG_FILE_NAME: &str = "ClearTemp.txt";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RemoveOption {
  Empty,
  None,
  Recurse,
  RemoveSelf,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PatternSet {
  pub(crate) negatives: Vec<String>,
  pub(crate) positives: Vec<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Rule {
  pub(crate) line: usize,
  pub(crate) option: RemoveOption,
  pub(crate) path: String,
  pub(crate) patterns: PatternSet,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Diagnostic {
  pub(crate) line: usize,
  pub(crate) message: String,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct ParsedConfig {
  pub(crate) diagnostics: Vec<Diagnostic>,
  pub(crate) rules: Vec<Rule>,
}

pub(crate) fn select_config_file(arguments: impl IntoIterator<Item = OsString>) -> Result<PathBuf, String> {
  let mut arguments = arguments.into_iter();
  match (arguments.next(), arguments.next()) {
    (Some(path), None) if !path.is_empty() => Ok(PathBuf::from(path)),
    (None, None) => {
      let current_directory = env::current_dir().map_err(|error| format!("Cannot determine the current directory: {error}"))?;
      let executable_directory = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(ToOwned::to_owned))
        .ok_or_else(|| "Cannot determine the executable directory.".to_owned())?;
      find_config_file(&current_directory, &executable_directory)
    },
    _ => Err("Usage: ClearTemp.exe [configuration-file]".to_owned()),
  }
}

pub(crate) fn find_config_file(current_directory: &Path, executable_directory: &Path) -> Result<PathBuf, String> {
  let mut directories = vec![current_directory];
  if path_key(current_directory) != path_key(executable_directory) {
    directories.push(executable_directory);
  }

  for directory in directories {
    let mut matches = fs::read_dir(directory)
      .map_err(|error| format!("Cannot inspect '{}': {error}", display_path(directory)))?
      .filter_map(Result::ok)
      .filter(|entry| entry.file_name().to_str().is_some_and(|name| name.eq_ignore_ascii_case(CONFIG_FILE_NAME)) && entry.path().is_file())
      .map(|entry| entry.path())
      .collect::<Vec<_>>();

    if matches.len() > 1 {
      return Err(format!("Multiple case-insensitive matches for '{CONFIG_FILE_NAME}' were found in '{}'.", display_path(directory)));
    }
    if let Some(path) = matches.pop() {
      return Ok(path);
    }
  }

  Err(format!("'{CONFIG_FILE_NAME}' was not found."))
}

pub(crate) fn load_config(path: &Path) -> Result<ParsedConfig, String> {
  let bytes = fs::read(path).map_err(|error| format!("Cannot read '{}': {error}", display_path(path)))?;
  let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
  let content = std::str::from_utf8(bytes).map_err(|error| format!("'{}' is not valid UTF-8: {error}", display_path(path)))?;
  Ok(parse_config(content))
}

pub(crate) fn parse_config(content: &str) -> ParsedConfig {
  let mut parsed = ParsedConfig::default();

  for (index, raw_line) in content.lines().enumerate() {
    let line_number = index + 1;
    let line = raw_line.trim();
    if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
      continue;
    }

    let parts = line.split('|').map(str::trim).collect::<Vec<_>>();
    if !(2..=3).contains(&parts.len()) {
      parsed.diagnostics.push(Diagnostic { line: line_number, message: "Expected '<path>|<patterns>[|<option>]'.".to_owned() });
      continue;
    }
    if parts[0].is_empty() || parts[1].is_empty() {
      parsed.diagnostics.push(Diagnostic { line: line_number, message: "Path and file patterns must not be empty.".to_owned() });
      continue;
    }

    let option = match parts.get(2).copied().unwrap_or("NONE").to_ascii_uppercase().as_str() {
      "EMPTY" => RemoveOption::Empty,
      "NONE" => RemoveOption::None,
      "RECURSE" => RemoveOption::Recurse,
      "REMOVESELF" => RemoveOption::RemoveSelf,
      _ => {
        parsed.diagnostics.push(Diagnostic { line: line_number, message: format!("Unknown removal option '{}'.", parts[2]) });
        continue;
      },
    };

    let patterns = match parse_patterns(parts[1]) {
      Ok(patterns) => patterns,
      Err(message) => {
        parsed.diagnostics.push(Diagnostic { line: line_number, message });
        continue;
      },
    };
    if option == RemoveOption::Empty && (patterns.positives != ["*"] || !patterns.negatives.is_empty()) {
      parsed.diagnostics.push(Diagnostic { line: line_number, message: "EMPTY requires the single file pattern '*' without exclusions.".to_owned() });
      continue;
    }
    parsed.rules.push(Rule { line: line_number, option, path: parts[0].replace('/', "\\"), patterns });
  }

  parsed
}

pub(crate) fn expand_path(pattern: &str) -> Result<Vec<PathBuf>, String> {
  let mut expanded = vec![String::new()];
  let mut remainder = pattern;

  while let Some(open) = remainder.find('%') {
    let after_open = &remainder[open + 1..];
    let close = after_open.find('%').ok_or_else(|| format!("Unclosed environment variable in '{pattern}'."))?;
    let name = &after_open[..close];
    if name.is_empty() {
      return Err(format!("Empty environment variable in '{pattern}'."));
    }
    let values = resolve_variable(name).ok_or_else(|| format!("Unknown or unavailable variable '%{name}%'."))?;
    let literal = &remainder[..open];
    expanded = expanded
      .into_iter()
      .flat_map(|prefix| {
        values.iter().map(move |value| {
          let mut result = prefix.clone();
          result.push_str(literal);
          result.push_str(value);
          result
        })
      })
      .collect();
    remainder = &after_open[close + 1..];
  }

  let mut seen = HashSet::new();
  Ok(
    expanded
      .into_iter()
      .map(|mut value| {
        value.push_str(remainder);
        PathBuf::from(value)
      })
      .filter(|path| seen.insert(path_key(path)))
      .collect(),
  )
}

fn parse_patterns(value: &str) -> Result<PatternSet, String> {
  let mut positives = Vec::new();
  let mut negatives = Vec::new();

  for token in value.split(';').map(str::trim) {
    if token.is_empty() {
      return Err("File patterns must not contain empty entries.".to_owned());
    }
    let (target, patterns) = if let Some(negative) = token.strip_prefix('!') { (negative.trim(), &mut negatives) } else { (token, &mut positives) };
    if target.is_empty() {
      return Err("An exclusion pattern must follow '!'.".to_owned());
    }
    if target.contains(['\\', '/', ':', '"', '<', '>', '|', '\0']) {
      return Err(format!("'{target}' is not a valid file-name pattern."));
    }
    patterns.push(target.to_owned());
  }

  if positives.is_empty() {
    positives.push("*".to_owned());
  }
  Ok(PatternSet { negatives, positives })
}

fn resolve_variable(name: &str) -> Option<Vec<String>> {
  let folders: &[&GUID] = if name.eq_ignore_ascii_case("ProgramFiles") {
    &[&FOLDERID_ProgramFilesX64, &FOLDERID_ProgramFilesX86]
  } else if name.eq_ignore_ascii_case("CommonProgramFiles") {
    &[&FOLDERID_ProgramFilesCommonX64, &FOLDERID_ProgramFilesCommonX86]
  } else {
    let folder = if name.eq_ignore_ascii_case("AppData") {
      &FOLDERID_RoamingAppData
    } else if name.eq_ignore_ascii_case("LocalAppData") {
      &FOLDERID_LocalAppData
    } else if name.eq_ignore_ascii_case("CommonAppData") || name.eq_ignore_ascii_case("ProgramData") {
      &FOLDERID_ProgramData
    } else if name.eq_ignore_ascii_case("Documents") {
      &FOLDERID_Documents
    } else if name.eq_ignore_ascii_case("LocalLowAppData") {
      &FOLDERID_LocalAppDataLow
    } else if name.eq_ignore_ascii_case("Music") {
      &FOLDERID_Music
    } else if name.eq_ignore_ascii_case("Pictures") {
      &FOLDERID_Pictures
    } else if name.eq_ignore_ascii_case("Video") {
      &FOLDERID_Videos
    } else if name.eq_ignore_ascii_case("Public") {
      &FOLDERID_Public
    } else if name.eq_ignore_ascii_case("UserProfile") {
      &FOLDERID_Profile
    } else if name.eq_ignore_ascii_case("WinDir") {
      &FOLDERID_Windows
    } else {
      return env::vars_os()
        .find_map(|(key, value)| key.to_str().is_some_and(|key| key.eq_ignore_ascii_case(name)).then(|| vec![value.to_string_lossy().into_owned()]));
    };
    &[folder]
  };

  let mut seen = HashSet::new();
  let values = folders.iter().filter_map(|folder| known_folder(folder).ok()).filter(|value| seen.insert(value.to_lowercase())).collect::<Vec<_>>();
  (!values.is_empty()).then_some(values)
}

fn known_folder(folder: &GUID) -> Result<String, String> {
  let mut pointer = ptr::null_mut();
  let result = unsafe { SHGetKnownFolderPath(folder, 0, ptr::null_mut(), &mut pointer) };
  if result < 0 {
    return Err(format!("SHGetKnownFolderPath failed with HRESULT 0x{result:08X}."));
  }

  let mut length = 0;
  unsafe {
    while *pointer.add(length) != 0 {
      length += 1;
    }
  }
  let value = unsafe { OsString::from_wide(std::slice::from_raw_parts(pointer, length)) };
  unsafe { CoTaskMemFree(pointer.cast()) };
  Ok(value.to_string_lossy().into_owned())
}

pub(crate) fn path_key(path: &Path) -> String {
  path.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase()
}

#[cfg(test)]
mod tests;
