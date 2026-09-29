use super::*;

#[test]
fn hides_verbatim_path_prefixes() {
  assert_eq!(display_path(Path::new(r"\\?\E:\Temp\file.tmp")), r"E:\Temp\file.tmp");
  assert_eq!(display_path(Path::new(r"\\?\UNC\server\share\file.tmp")), r"\\server\share\file.tmp");
}

#[test]
fn classifies_common_windows_errors() {
  assert!(matches!(classify(&io::Error::from_raw_os_error(5)), ResultKind::NoPerm));
  assert!(matches!(classify(&io::Error::from_raw_os_error(32)), ResultKind::Locked));
  assert!(matches!(classify(&io::Error::from_raw_os_error(145)), ResultKind::NotEmpty));
}

#[test]
fn accepts_only_key_down_without_modifiers() {
  assert!(is_unmodified_key_down(true, 0));
  assert!(!is_unmodified_key_down(false, 0));
  for modifier in [SHIFT_PRESSED, LEFT_CTRL_PRESSED, RIGHT_CTRL_PRESSED, LEFT_ALT_PRESSED, RIGHT_ALT_PRESSED] {
    assert!(!is_unmodified_key_down(true, modifier));
  }
}
