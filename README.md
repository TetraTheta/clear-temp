# ClearTemp

ClearTemp is a small Windows command-line program that permanently removes files described by an editable `ClearTemp.txt` configuration file. Its rule values follow the Winapp2 `FileKey` path, pattern, and recursion model without accepting a complete `winapp2.ini` file.

> [!WARNING]
> ClearTemp deletes files directly. It does not use the Recycle Bin and requests administrator privileges by default. Review every rule before running it.

## Requirements and build

- Windows 10 or Windows 11 on x64
- The stable Rust MSVC toolchain
- A Windows SDK available to the MSVC resource compiler

```powershell
cargo build --release
```

The executable and a copy of the default configuration are written to `target\release\ClearTemp.exe` and `target\release\ClearTemp.txt`.

Run ClearTemp with optional launch options and zero or one configuration file:

```text
ClearTemp.exe [--no-elevate] [--no-wait] [configuration-file]
```

| Launch option | Behavior |
|---|---|
| `--no-elevate` | Does not request administrator privileges. If ClearTemp was started by an already elevated process, it remains elevated. |
| `--no-wait` | Does not print `Press any non-modifier key to continue...` or wait for keyboard input before exiting. |

By default, ClearTemp requests administrator privileges through Windows User Account Control (UAC). This allows rules to clean protected locations. Use `--no-elevate` when a script should run ClearTemp with its current permissions and report inaccessible files normally.

For scripts that must continue without keyboard interaction, use `--no-wait`:

```bat
ClearTemp.exe --no-wait
ClearTemp.exe --no-elevate --no-wait ".\configs\User Cleanup.txt"
```

When a configuration file argument is provided, ClearTemp reads that file instead of searching for `ClearTemp.txt`. Relative paths are resolved from the current working directory. Quote paths containing spaces:

```powershell
.\ClearTemp.exe ".\configs\Browser Cleanup.txt"
```

Only one configuration file is accepted. Unknown options or additional file arguments produce a usage error without starting any cleaning operation. Use `--` before a configuration file whose name begins with `-`.

Without an argument, ClearTemp searches for the configuration in this order:

1. The current working directory.
2. The directory containing `ClearTemp.exe`.

The file name comparison is case-insensitive. Names such as `ClearTemp.txt`, `cleartemp.txt`, and `cLEARtEMP.txt` are equivalent. If a case-sensitive directory contains more than one matching name, ClearTemp stops instead of choosing one arbitrarily.

The configuration must be UTF-8, with or without a UTF-8 byte-order mark (BOM).

## Configuration syntax

Each non-comment line is one cleaning rule containing a Winapp2 `FileKey` value:

```text
rule          = directory, "|", patterns, [ "|", option ];
patterns      = pattern, { ";", pattern };
pattern       = [ "!" ], file-name-pattern;
option        = "NONE" | "RECURSE" | "EMPTY" | "REMOVESELF";
```

In practical form:

```text
<directory path>|<file patterns>[|NONE|RECURSE|EMPTY|REMOVESELF]
```

| Field | Required | Meaning |
|---|---:|---|
| Directory path | Yes | Directory to clean. Environment variables and `*`/`?` wildcards may appear in any path segment. |
| File patterns | Yes | One or more semicolon-separated file-name patterns. Patterns apply to files, not directory names. |
| Option | No | Cleaning depth and directory removal behavior. The default is `NONE`. |

Leading and trailing whitespace around fields and individual patterns is ignored. Blank lines are ignored. A line whose first non-whitespace character is `;` or `#` is a comment. Inline comments are not supported because `;` separates file patterns.

Malformed rules are reported with their line number and skipped. Other valid rules continue to run. Empty fields, more than three fields, unknown options, unavailable variables, and invalid file-name patterns are malformed.

ClearTemp accepts only the value portion of a Winapp2 `FileKey`. Do not include INI sections or key names:

```text
; Valid
%TEMP%|*.tmp|RECURSE

; Invalid
FileKey1=%TEMP%|*.tmp|RECURSE
```

### Wildcards

`*` matches zero or more characters and `?` matches one character using native Windows matching rules. Directory wildcards are expanded one path segment at a time, so nested wildcards are supported:

```text
%LocalAppData%\Vendor*\Product\User *\Cache|*|REMOVESELF
```

File patterns are case-insensitive on Windows. Multiple positive patterns can be separated with semicolons:

```text
%TEMP%|*.tmp;*.log;crash-?.dmp|RECURSE
```

A pattern beginning with `!` excludes matching files. This is a ClearTemp compatibility extension, not standard Winapp2 syntax. If a rule contains only exclusions, `*` is used as the implicit positive pattern:

```text
%TEMP%|*;!keep.tmp;!important-*.log|RECURSE
%TEMP%|!keep-*|NONE
```

The leading `!` is reserved and cannot be escaped to target a literal file name beginning with `!`.

### Removal options

| Option | Behavior |
|---|---|
| `NONE` | Deletes matching files directly inside each matched directory. Subdirectories are not visited. |
| `RECURSE` | Deletes matching files in the matched directory and all real subdirectories. Directories are retained. |
| `EMPTY` | Deletes every file and subdirectory below the matched directory while retaining the matched directory itself. |
| `REMOVESELF` | Performs recursive cleaning, then removes empty subdirectories and the matched directory. A directory containing an unmatched or excluded file is retained. |

Options are case-insensitive. `REMOVESELF` implies `RECURSE`, matching the Winapp2 `FileKey` model.

`EMPTY` is a ClearTemp extension for emptying a directory without removing that directory. It requires the single pattern `*` without exclusions:

```text
%TEMP%|*|EMPTY
```

`RECURSE` is not equivalent: it deletes matching files throughout the tree but retains every directory.

ClearTemp never follows directory symbolic links, junctions, or other directory reparse points. `RECURSE` and `REMOVESELF` skip them; `EMPTY` removes the reparse point itself without modifying its linked target.

## Environment variables and Winapp2 aliases

Variable names are case-insensitive and use `%Name%` syntax. Ordinary Windows environment variables, including `%TEMP%` and `%TMP%`, are supported even when they are not listed below.

ClearTemp also resolves the standard Winapp2 locations below. User folders are obtained through the Windows Known Folder API, so redirected folders use their actual configured locations.

| Variable | Resolution |
|---|---|
| `%AppData%` | Current user's roaming application data folder. |
| `%LocalAppData%` | Current user's local application data folder. |
| `%CommonAppData%` | Alias for the shared ProgramData folder. |
| `%Documents%` | Current user's Documents known folder. |
| `%LocalLowAppData%` | Current user's LocalLow application data folder. |
| `%Music%` | Current user's Music known folder. |
| `%Pictures%` | Current user's Pictures known folder. |
| `%Video%` | Current user's Videos known folder. |
| `%ProgramData%` | Shared application data folder. |
| `%ProgramFiles%` | Both x64 and x86 Program Files folders. |
| `%CommonProgramFiles%` | Both x64 and x86 Common Files folders. |
| `%Public%` | Public user profile folder. |
| `%SystemDrive%` | Windows system drive, normally `C:`. |
| `%UserProfile%` | Current user's profile folder. |
| `%WinDir%` | Windows installation folder. |

`%ProgramFiles%` and `%CommonProgramFiles%` can expand one rule into two candidate paths. Expansion is performed before wildcard matching. After wildcard matching, ClearTemp removes duplicate operations with the same case-insensitive directory, patterns, and removal option, then starts cleaning. Rules with different patterns or removal options are kept and run in configuration order.

For example, this rule checks both native x64 and x86 installation roots:

```text
%ProgramFiles%\Example App\Cache|*|REMOVESELF
```

## Browser example

These rules demonstrate the nested path wildcards used by Winapp2 browser entries:

```text
%LocalAppData%\Google\Chrome*\User Data\*|*Web Data
%LocalAppData%\Google\Chrome*\User Data\*\AutoFill*|*|REMOVESELF
%LocalAppData%\Google\Chrome*\User Data\AutoFill*|*|REMOVESELF
```

1. The first rule finds every matching Chrome installation and profile directory, then deletes files whose names end with `Web Data` directly inside each profile.
2. The second rule finds `AutoFill*` directories inside every profile, recursively cleans them, and removes directories that become empty.
3. The third rule handles `AutoFill*` directories located directly below `User Data` in the same way.

## Long paths and output

ClearTemp is long-path aware and may use Windows verbatim paths such as `\\?\E:\...` internally. Console output always removes that implementation prefix and displays normal drive or UNC paths, such as `E:\...` or `\\server\share\...`.

Individual file and directory errors are classified and printed without stopping later work. Configuration discovery, reading, or encoding failures stop the program before cleaning starts.

## References

- [Winapp2 `FileKey` syntax](https://github.com/MoscaDotTo/Winapp2/blob/master/CONTRIBUTING.md#filekey-example)
- [Winapp2 environment variables](https://github.com/MoscaDotTo/Winapp2/blob/master/CONTRIBUTING.md#environment-variables)

