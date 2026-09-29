use std::io::{self, IsTerminal, Write};
use std::path::Path;

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Console::{
  CONSOLE_SCREEN_BUFFER_INFO, FOREGROUND_GREEN, FOREGROUND_INTENSITY, FOREGROUND_RED, GetConsoleScreenBufferInfo, GetStdHandle, INPUT_RECORD,
  KEY_EVENT, LEFT_ALT_PRESSED, LEFT_CTRL_PRESSED, RIGHT_ALT_PRESSED, RIGHT_CTRL_PRESSED, ReadConsoleInputW, SHIFT_PRESSED, STD_INPUT_HANDLE,
  STD_OUTPUT_HANDLE, SetConsoleTextAttribute,
};

const MODIFIER_KEYS: u32 = LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED | LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED | SHIFT_PRESSED;

#[derive(Clone, Copy)]
enum ResultKind {
  BadPath,
  Error,
  Info,
  IoError,
  Locked,
  NoPerm,
  NotEmpty,
  NotFound,
  Skipped,
  Success,
  TooLong,
  Warning,
}

pub(crate) fn print_info(path: &Path) {
  print_result(ResultKind::Info, &display_path(path));
}

pub(crate) fn print_success(path: &Path) {
  print_result(ResultKind::Success, &display_path(path));
}

pub(crate) fn print_skipped(path: &Path, reason: &str) {
  print_result(ResultKind::Skipped, &format!("{} ({reason})", display_path(path)));
}

pub(crate) fn print_warning(message: &str) {
  print_result(ResultKind::Warning, message);
}

pub(crate) fn print_error_message(message: &str) {
  print_result(ResultKind::Error, message);
}

pub(crate) fn print_error(error: &io::Error, path: &Path) {
  let kind = classify(error);
  print_result(kind, &display_path(path));
  if matches!(kind, ResultKind::IoError | ResultKind::BadPath | ResultKind::Error) {
    println!("      {error}");
  }
}

pub(crate) fn display_path(path: &Path) -> String {
  let value = path.as_os_str().to_string_lossy();
  if let Some(unc) = value.strip_prefix("\\\\?\\UNC\\") {
    format!("\\\\{unc}")
  } else if let Some(normal) = value.strip_prefix("\\\\?\\") {
    normal.to_owned()
  } else {
    value.into_owned()
  }
}

pub(crate) fn wait_for_key() {
  if !io::stdin().is_terminal() {
    return;
  }
  println!();
  println!("Press any non-modifier key to continue...");

  let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
  if handle.is_null() || handle == INVALID_HANDLE_VALUE {
    return;
  }
  loop {
    let mut record = INPUT_RECORD::default();
    let mut read = 0;
    if unsafe { ReadConsoleInputW(handle, &mut record, 1, &mut read) } == 0 {
      return;
    }
    if read == 1 && record.EventType == KEY_EVENT as u16 {
      let key = unsafe { record.Event.KeyEvent };
      if is_unmodified_key_down(key.bKeyDown != 0, key.dwControlKeyState) {
        return;
      }
    }
  }
}

fn is_unmodified_key_down(key_down: bool, control_state: u32) -> bool {
  key_down && control_state & MODIFIER_KEYS == 0
}

fn print_result(kind: ResultKind, value: &str) {
  let (label, color) = match kind {
    ResultKind::Info => ("[PROCESSING] ", FOREGROUND_GREEN),
    ResultKind::Success => ("[DELETED   ] ", FOREGROUND_GREEN),
    ResultKind::Locked => ("[LOCKED    ] ", FOREGROUND_RED),
    ResultKind::NotEmpty => ("[NOT EMPTY ] ", FOREGROUND_RED),
    ResultKind::NoPerm => ("[NO PERM   ] ", FOREGROUND_RED),
    ResultKind::NotFound => ("[NOT FOUND ] ", FOREGROUND_RED | FOREGROUND_GREEN),
    ResultKind::TooLong => ("[LONG PATH ] ", FOREGROUND_RED),
    ResultKind::BadPath => ("[BAD PATH  ] ", FOREGROUND_RED),
    ResultKind::IoError => ("[IO ERROR  ] ", FOREGROUND_RED),
    ResultKind::Skipped => ("[SKIPPED   ] ", FOREGROUND_RED | FOREGROUND_GREEN),
    ResultKind::Warning => ("[WARNING   ] ", FOREGROUND_RED | FOREGROUND_GREEN),
    ResultKind::Error => ("[ERROR     ] ", FOREGROUND_RED),
  };

  write_colored(label, color);
  if let Some((prefix, name)) = value.rsplit_once('\\') {
    print!("{prefix}\\");
    let _ = io::stdout().flush();
    write_colored(name, FOREGROUND_RED | FOREGROUND_GREEN | FOREGROUND_INTENSITY);
    println!();
  } else {
    write_colored(value, FOREGROUND_RED | FOREGROUND_GREEN | FOREGROUND_INTENSITY);
    println!();
  }
}

fn write_colored(value: &str, color: u16) {
  if !io::stdout().is_terminal() {
    print!("{value}");
    return;
  }
  let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
  let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
  if handle.is_null() || handle == INVALID_HANDLE_VALUE || unsafe { GetConsoleScreenBufferInfo(handle, &mut info) } == 0 {
    print!("{value}");
    return;
  }

  let _ = io::stdout().flush();
  unsafe {
    SetConsoleTextAttribute(handle, color);
  }
  print!("{value}");
  let _ = io::stdout().flush();
  unsafe {
    SetConsoleTextAttribute(handle, info.wAttributes);
  }
}

fn classify(error: &io::Error) -> ResultKind {
  match error.raw_os_error() {
    Some(2 | 3) => ResultKind::NotFound,
    Some(5) => ResultKind::NoPerm,
    Some(32 | 33 | 1224) => ResultKind::Locked,
    Some(123 | 161) => ResultKind::BadPath,
    Some(145) => ResultKind::NotEmpty,
    Some(206) => ResultKind::TooLong,
    _ => match error.kind() {
      io::ErrorKind::NotFound => ResultKind::NotFound,
      io::ErrorKind::PermissionDenied => ResultKind::NoPerm,
      io::ErrorKind::InvalidInput => ResultKind::BadPath,
      _ => ResultKind::IoError,
    },
  }
}

#[cfg(test)]
mod tests;
