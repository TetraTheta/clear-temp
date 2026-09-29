use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_FILES, ERROR_PATH_NOT_FOUND, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
  FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FindClose, FindFirstFileW, FindNextFileW, WIN32_FIND_DATAW,
};

use crate::configuration::{PatternSet, RemoveOption, path_key};
use crate::console::{print_error, print_skipped, print_success};

#[derive(Debug)]
struct FoundEntry {
  attributes: u32,
  name: OsString,
  path: PathBuf,
}

struct FindHandle(HANDLE);

impl Drop for FindHandle {
  fn drop(&mut self) {
    unsafe {
      FindClose(self.0);
    }
  }
}

pub(crate) fn expand_directories(patterns: &[PathBuf]) -> Vec<PathBuf> {
  let mut directories = Vec::new();
  let mut seen = HashSet::new();

  for pattern in patterns {
    match expand_directory_pattern(pattern) {
      Ok(paths) => {
        directories.extend(paths.into_iter().filter(|path| seen.insert(path_key(path))));
      },
      Err(error) => print_error(&error, pattern),
    }
  }

  directories
}

pub(crate) fn process(path: &Path, patterns: &PatternSet, option: RemoveOption) {
  match option {
    RemoveOption::None => delete_matching_files(path, patterns),
    RemoveOption::Empty | RemoveOption::Recurse | RemoveOption::RemoveSelf => clear_recursive(path, patterns, option, true),
  }
}

fn expand_directory_pattern(pattern: &Path) -> io::Result<Vec<PathBuf>> {
  let absolute = std::path::absolute(pattern)?;
  let mut base = PathBuf::new();
  let mut segments = Vec::new();

  for component in absolute.components() {
    match component {
      Component::Prefix(prefix) => base.push(prefix.as_os_str()),
      Component::RootDir => base.push(Path::new("\\")),
      Component::Normal(segment) => segments.push(segment.to_owned()),
      Component::CurDir => {},
      Component::ParentDir => {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Parent path components are not supported"));
      },
    }
  }

  let mut bases = vec![base];
  for segment in segments {
    let mut next = Vec::new();
    let mut seen = HashSet::new();
    for current in bases {
      for entry in find_entries(&current.join(&segment))? {
        if !entry.is_directory() {
          continue;
        }
        if entry.is_reparse_point() {
          print_skipped(&entry.path, "reparse point");
          continue;
        }
        if seen.insert(path_key(&entry.path)) {
          next.push(entry.path);
        }
      }
    }
    if next.is_empty() {
      return Ok(Vec::new());
    }
    bases = next;
  }

  Ok(bases)
}

fn clear_recursive(path: &Path, patterns: &PatternSet, option: RemoveOption, is_target: bool) {
  delete_matching_files(path, patterns);

  match find_entries(&path.join("*")) {
    Ok(entries) => {
      for entry in entries.into_iter().filter(FoundEntry::is_directory) {
        if entry.is_reparse_point() {
          if option == RemoveOption::Empty {
            remove_directory(&entry.path);
          } else {
            print_skipped(&entry.path, "reparse point");
          }
        } else {
          clear_recursive(&entry.path, patterns, option, false);
        }
      }
    },
    Err(error) => print_error(&error, path),
  }

  if option == RemoveOption::RemoveSelf || (option == RemoveOption::Empty && !is_target) {
    remove_directory(path);
  }
}

fn remove_directory(path: &Path) {
  match fs::remove_dir(to_verbatim(path)) {
    Ok(()) => print_success(path),
    Err(error) => print_error(&error, path),
  }
}

fn delete_matching_files(path: &Path, patterns: &PatternSet) {
  let excluded = collect_matching_files(path, &patterns.negatives);
  let files = collect_matching_files(path, &patterns.positives);

  for (name, file) in files {
    if excluded.contains_key(&name) {
      continue;
    }
    match fs::remove_file(to_verbatim(&file)) {
      Ok(()) => print_success(&file),
      Err(error) => print_error(&error, &file),
    }
  }
}

fn collect_matching_files(path: &Path, patterns: &[String]) -> HashMap<OsString, PathBuf> {
  let mut files = HashMap::new();
  for pattern in patterns {
    match find_entries(&path.join(pattern)) {
      Ok(entries) => {
        for entry in entries.into_iter().filter(|entry| !entry.is_directory()) {
          files.entry(entry.name).or_insert(entry.path);
        }
      },
      Err(error) => print_error(&error, path),
    }
  }
  files
}

fn find_entries(search: &Path) -> io::Result<Vec<FoundEntry>> {
  let wide = wide_null(&to_verbatim(search));
  let mut data = WIN32_FIND_DATAW::default();
  let raw_handle = unsafe { FindFirstFileW(wide.as_ptr(), &mut data) };
  if raw_handle == INVALID_HANDLE_VALUE {
    let error = unsafe { GetLastError() };
    if error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND {
      return Ok(Vec::new());
    }
    return Err(io::Error::from_raw_os_error(error as i32));
  }
  let _handle = FindHandle(raw_handle);
  let parent = search.parent().unwrap_or_else(|| Path::new(""));
  let mut entries = Vec::new();

  loop {
    let length = data.cFileName.iter().position(|character| *character == 0).unwrap_or(data.cFileName.len());
    let name = OsString::from_wide(&data.cFileName[..length]);
    if name != OsStr::new(".") && name != OsStr::new("..") {
      entries.push(FoundEntry { attributes: data.dwFileAttributes, path: parent.join(&name), name });
    }

    if unsafe { FindNextFileW(raw_handle, &mut data) } == 0 {
      let error = unsafe { GetLastError() };
      if error != ERROR_NO_MORE_FILES {
        return Err(io::Error::from_raw_os_error(error as i32));
      }
      break;
    }
  }

  Ok(entries)
}

impl FoundEntry {
  fn is_directory(&self) -> bool {
    self.attributes & FILE_ATTRIBUTE_DIRECTORY != 0
  }

  fn is_reparse_point(&self) -> bool {
    self.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
  }
}

pub(crate) fn to_verbatim(path: &Path) -> PathBuf {
  let value = path.as_os_str().to_string_lossy();
  if value.starts_with("\\\\?\\") {
    return path.to_path_buf();
  }
  if let Some(unc) = value.strip_prefix("\\\\") {
    return PathBuf::from(format!("\\\\?\\UNC\\{unc}"));
  }
  if path.is_absolute() {
    return PathBuf::from(format!("\\\\?\\{value}"));
  }
  path.to_path_buf()
}

fn wide_null(value: &Path) -> Vec<u16> {
  value.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests;
