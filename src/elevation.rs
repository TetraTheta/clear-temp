use std::env;
use std::ffi::{OsStr, OsString};
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::process::ExitCode;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_FAILED};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{IsUserAnAdmin, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub(crate) fn is_elevated() -> bool {
  unsafe { IsUserAnAdmin() != 0 }
}

pub(crate) fn run_elevated(arguments: &[OsString]) -> io::Result<ExitCode> {
  let executable_path = env::current_exe()?;
  let executable = wide_null(executable_path.as_os_str());
  let parameters = command_line(arguments.iter().map(OsString::as_os_str));
  let directory_path = env::current_dir()?;
  let directory = wide_null(directory_path.as_os_str());
  let verb = wide_null(OsStr::new("runas"));
  let mut execute = SHELLEXECUTEINFOW {
    cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
    fMask: SEE_MASK_NOCLOSEPROCESS,
    lpVerb: verb.as_ptr(),
    lpFile: executable.as_ptr(),
    lpParameters: parameters.as_ptr(),
    lpDirectory: directory.as_ptr(),
    nShow: SW_SHOWNORMAL,
    ..Default::default()
  };

  if unsafe { ShellExecuteExW(&mut execute) } == 0 {
    return Err(io::Error::last_os_error());
  }
  let process = ProcessHandle(execute.hProcess);
  if process.0.is_null() {
    return Err(io::Error::other("elevated process handle was not returned"));
  }
  if unsafe { WaitForSingleObject(process.0, INFINITE) } == WAIT_FAILED {
    return Err(io::Error::last_os_error());
  }

  let mut exit_code = 0;
  if unsafe { GetExitCodeProcess(process.0, &mut exit_code) } == 0 {
    return Err(io::Error::last_os_error());
  }
  Ok(if exit_code == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

struct ProcessHandle(HANDLE);

impl Drop for ProcessHandle {
  fn drop(&mut self) {
    if !self.0.is_null() {
      unsafe {
        CloseHandle(self.0);
      }
    }
  }
}

fn command_line<'a>(arguments: impl IntoIterator<Item = &'a OsStr>) -> Vec<u16> {
  let mut command_line = Vec::new();
  for argument in arguments {
    if !command_line.is_empty() {
      command_line.push(b' ' as u16);
    }
    append_quoted(&mut command_line, argument);
  }
  command_line.push(0);
  command_line
}

fn append_quoted(command_line: &mut Vec<u16>, argument: &OsStr) {
  let value = argument.encode_wide().collect::<Vec<_>>();
  if !value.is_empty() && !value.iter().any(|character| matches!(*character, 0x09 | 0x20 | 0x22)) {
    command_line.extend(value);
    return;
  }

  command_line.push(b'"' as u16);
  let mut backslashes = 0;
  for character in value {
    if character == b'\\' as u16 {
      backslashes += 1;
    } else {
      command_line.extend(std::iter::repeat_n(b'\\' as u16, backslashes));
      if character == b'"' as u16 {
        command_line.extend(std::iter::repeat_n(b'\\' as u16, backslashes + 1));
      }
      command_line.push(character);
      backslashes = 0;
    }
  }
  command_line.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2));
  command_line.push(b'"' as u16);
}

fn wide_null(value: &OsStr) -> Vec<u16> {
  value.encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn quotes_elevated_process_arguments() {
    assert_eq!(render([OsStr::new("--no-wait"), OsStr::new("rules.txt")]), "--no-wait rules.txt");
    assert_eq!(render([OsStr::new(r"C:\rules folder\")]), r#""C:\rules folder\\""#);
    assert_eq!(render([OsStr::new("a\"b")]), r#""a\"b""#);
    assert_eq!(render([OsStr::new("")]), r#""""#);
  }

  fn render<'a>(arguments: impl IntoIterator<Item = &'a OsStr>) -> String {
    let command_line = command_line(arguments);
    String::from_utf16(&command_line[..command_line.len() - 1]).unwrap()
  }
}
