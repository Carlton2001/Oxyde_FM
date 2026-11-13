# Windows checks since v2.2.1

What to try on Windows before the next release: every change since tag `v2.2.1` (up to `21709a9a`) that reaches Windows. Unlike the other guides, this is a release checklist: remove it, or replace it with the next one, once the release is tagged. Report each check as passed, failed (with steps and the log) or not tried. General practice: [visual checks](visual-checks.md).

Not concerned, Linux only: the GPU the compositor renders with (`8d886241`, `eaf81f36`, apart from the quit from the event loop, which cannot be provoked by hand), `fstab` mounts (`0d1bb1e0`), `install.sh` uninstalling a running Oxyde (`14f164ac`), the Wayland activated state and resize throttle (`fd4957c5`) and the Linux window frame (shadow, grips, move threshold of `9e53b8e9`).

## Preparation

Close every Oxyde, then from the repository root, in PowerShell:

```powershell
cargo build --offline --bins
cargo build --offline --manifest-path shell/oxyde-file-dialog/Cargo.toml --target-dir target
cargo run --offline
```

Automated suites first, so the manual pass starts from green:

```powershell
./scripts/test-windows.ps1 -Offline
cargo test --offline --manifest-path shell/oxyde-file-dialog/Cargo.toml --target-dir target
```

Installed copy, for the checks marked **Setup** (system integration from an installed copy, updates):

```powershell
./scripts/build-distribution.ps1
```

Fixtures, in a private folder, never on personal documents:

```powershell
$check = "$env:USERPROFILE\OxydeCheck"
New-Item -ItemType Directory -Force "$check\src", "$check\dst" | Out-Null
# A large file, for operations long enough to pause, cancel or kill (8 GB here).
fsutil file createnew "$check\src\big.bin" 8589934592
# Many small files, for the items table of the Operations window.
New-Item -ItemType Directory -Force "$check\src\many" | Out-Null
1..2000 | ForEach-Object { Set-Content "$check\src\many\file $_.txt" "line $_" }
```

The log is `%APPDATA%\Oxyde\oxyde.log` (`data\oxyde.log` next to a portable executable): `notepad "$env:APPDATA\Oxyde\oxyde.log"`.

Run the interface checks at least in a light and a dark theme, at 100% and 150% display scale, and once in French and German (longest labels). With two screens, keep one at another scale.

## 1. Open and Save dialogs (experimental)

Commits `87e4d1e4`, `d65868c4`, `5f03ee61`, `21709a9a`. Reference: [system integration](../architecture/system-integration.md#windows-dialogs-experimental).

### 1.1 Turning it on and off

1. Preferences, **System integration**, check **Open and Save dialogs (experimental)**.
2. The two classes point to the library next to the executable:

   ```powershell
   reg query "HKCU\Software\Classes\CLSID\{DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7}\InprocServer32"
   reg query "HKCU\Software\Classes\CLSID\{C0B4E2F3-BA21-4773-8DBA-335EC946EB8B}\InprocServer32"
   ```

3. Unchecked, both queries fail (`ERROR: The system was unable to find the specified registry key`) and applications get Windows' dialog again, from their next dialog on.
4. **Setup**: same from the installed copy; uninstalling withdraws the registration before removing the files.

An application that showed a dialog keeps `target\debug\oxyde_file_dialog.dll` loaded until it ends: close it before rebuilding the library. Uncheck the option before deleting `target`.

### 1.2 Showing the dialogs without another application

`shell/oxyde-file-dialog/examples/show_dialog.rs` asks for the dialogs as an application does, through the registered classes: Oxyde's chooser with the option checked, Windows' dialog without. It works in `%TEMP%\oxyde-dialog-check` (files of several types, `read-only.txt`, `hidden.txt` with the hidden attribute, `Subfolder`), and prints what the dialog answered: paths, type filter index, folder and the state of each control.

```powershell
# One case, or several in turn; no case is "open".
cargo run --offline --manifest-path shell/oxyde-file-dialog/Cargo.toml --target-dir target --example show_dialog -- controls
cargo run --offline --manifest-path shell/oxyde-file-dialog/Cargo.toml --target-dir target --example show_dialog -- all
cargo run --offline --manifest-path shell/oxyde-file-dialog/Cargo.toml --target-dir target --example show_dialog -- --help
# Once built, without Cargo:
target\debug\examples\show_dialog.exe save-strict
```

To see what the library was asked and why it left a dialog to Windows, set its log in the same PowerShell session first:

```powershell
$env:OXYDE_FILE_DIALOG_LOG = "$env:TEMP\oxyde-file-dialog.log"
target\debug\examples\show_dialog.exe push-button
Get-Content $env:OXYDE_FILE_DIALOG_LOG -Tail 20
```

| Case | What it asks | Expected in Oxyde's chooser |
| --- | --- | --- |
| `open` | One file, folder set | Opens on the check folder, title "Oxyde check: open"; the chosen path is printed. Cancel prints `cancelled`. |
| `multiple` | Several files, three type filters | Ctrl, Shift, Ctrl+A, the selection rectangle; every path printed. |
| `folder` | `FOS_PICKFOLDERS`, several | Only folders listed; the button reads Select; a typed file name is refused. |
| `filters` | Text, Images, All files; the second shown first | Type menu on Images; switching filters the list, folders always listed; the printed index follows the menu. |
| `controls` | Combo (one item disabled), radio list, two check buttons, an edit box named by the text before it, a disabled check button, a note, the open drop-down | Form below the list: Encoding menu without "Hidden item", "Line endings" menu, two checkboxes, an "Author" text field holding "Ada", a faded "Locked by policy" left out of Tab, the note muted, and a chevron beside the button with Open, Open read-only, Open a copy. Every answer changed in Oxyde comes back printed; the button stands for the drop-down item last picked. |
| `refuse` | A listener refusing the first choice | `OnFileOk(…): refused` printed, Oxyde's chooser shows again, the second choice is accepted. |
| `options` | Labels "Document" and "Leave", button "Take", hidden files, needs interaction, hidden pins, places added at top and bottom, a default folder, a client GUID | Name label and Cancel read as given; `hidden.txt` listed; the button stays disabled until a folder change, a press in the list, a keyboard move or a typed name; the sidebar has no pinned folders, the check folder before Home and `Subfolder` after the places; opens on the default folder the first time. |
| `client-a`, `client-b` | Two client GUIDs, no folder | Choose in a different folder under each; the next `client-a` opens where `client-a` last chose, the next `client-b` where it did. |
| `save` | Name "New notes", default extension `txt`, filters | Name field first with the stem selected; saving without extension adds that of the shown filter; an existing name asks before replacing (prompt owned by the chooser: Cancel first, Tab, Enter, Esc). |
| `save-again` | `SetSaveAsItem` on `notes.txt` | Opens in its folder with `notes.txt` as name; saving asks to replace. |
| `save-strict` | `FOS_STRICTFILETYPES`, no overwrite prompt, Images shown, name "picture", labels "Export as" and "Export" | `picture.png` saved; typing `picture.txt` under Images still saves with `.png` added; an existing file is replaced without prompt; `read-only.txt` is refused (the field shakes). |
| `many-controls` | Save with filters, six combo boxes, three check buttons | Menus side by side when the window is wide, wrapping when narrowed; past a third of the window the form scrolls with the shared scrollbar; Tab follows the reading order and scrolls the form to the control reached. |
| `push-button` | A push button | Windows' dialog shows, not Oxyde's. |
| `navigation-root` | `SetNavigationRoot` | Windows' dialog shows. |
| `non-storage` | `FOS_ALLNONSTORAGEITEMS` | Windows' dialog shows. |

Run the cases both with Oxyde running and with every Oxyde closed: the chooser then starts alone (no main window) and the process ends with it. While the chooser is shown, the terminal window that owns it does not take input. Killing Oxyde while a chooser is shown (`taskkill /F /IM oxyde.exe`) must leave the example answered (cancelled or Windows' dialog), never hung.

### 1.3 The chooser window

Through any case above (`multiple` and `save` cover most):

- **Navigation bar**: Back, Forward, Up (disabled at a drive root), New folder; crumbs separated by `\`, the separator button keeping its fill while its menu is open; right click on a crumb, **Copy path**, then paste: the path is in the clipboard.
- **New folder** (button and Ctrl+Shift+N): "New folder", then "New folder (2)", edited in place; Enter keeps the name, Esc and a click elsewhere end the edit; a refused name (`a<b`) frames the field in red and shakes it. In the main window, Undo removes the folder.
- **Path field**: double click in the blank area, Ctrl+L, `/` or `~` in the list; a typed folder opens, a typed file is chosen. Enter in the path, name and filter fields never crashes.
- **Sidebar**: drives show a chevron; the chevron or a double press unfolds, a press opens; the tree unfolds to the folder open and centers it; hidden and system folders dimmed but openable; unfolding here leaves the main window's sidebar as it was; a first folder created in a folder gives it a chevron. Double click on the sidebar edge fits it to its widest row.
- **List**: with `multiple`, the selection rectangle from the blank area (Shift adds, Ctrl toggles, the list scrolls near its edges) and tooltips hidden while drawing; Ctrl+Shift+A selects nothing; a press in a scrollbar's track moves the thumb there; the button back to the start beside the rail.
- **Form and footer**: the questions sit below the list on the inactive panels' background, labels in one column measured in the language shown, Type label before the filters; the footer holds only Cancel and the button. Tab and Shift+Tab go from the list through the form and the footer with focus rings, Enter or Space presses, Esc cancels.
- **Memory**: view, sort, sidebar width and last folder survive closing the chooser.

### 1.4 Real applications

The example is the reference; a few real applications confirm the library behaves inside other processes. PowerShell's .NET dialogs use the same classes (64-bit PowerShell only: 32-bit applications are not covered by design):

```powershell
Add-Type -AssemblyName System.Windows.Forms
$open = New-Object System.Windows.Forms.OpenFileDialog
$open.Multiselect = $true
$open.Filter = 'Text (*.txt;*.md)|*.txt;*.md|Images (*.png;*.jpg)|*.png;*.jpg|All files (*.*)|*.*'
if ($open.ShowDialog() -eq 'OK') { $open.FileNames }

$save = New-Object System.Windows.Forms.SaveFileDialog
$save.Filter = 'Text (*.txt)|*.txt|All files (*.*)|*.*'
$save.FileName = 'notes'
if ($save.ShowDialog() -eq 'OK') { $save.FileName }
```

In PowerShell 7.4 or later (`pwsh`, .NET 8), the folder picker and the newer options go through the same dialog:

```powershell
Add-Type -AssemblyName System.Windows.Forms
$folder = New-Object System.Windows.Forms.FolderBrowserDialog
$folder.Multiselect = $true
if ($folder.ShowDialog() -eq 'OK') { $folder.SelectedPaths }

$open = New-Object System.Windows.Forms.OpenFileDialog
$open.ShowHiddenFiles = $true; $open.OkRequiresInteraction = $true; $open.ShowPinnedPlaces = $false
$open.ClientGuid = [guid]'6f0d3b52-4c1e-4a8e-9a3b-1f6e2c7d8a04'
$open.CustomPlaces.Add("$env:USERPROFILE\OxydeCheck")
if ($open.ShowDialog() -eq 'OK') { $open.FileName }
```

Then a few desktop applications in daily use (an editor's Open and Save As, a browser's upload and download). Expected: Oxyde's chooser, or Windows' dialog where the application adds something Oxyde does not show; the log above says which. Files chosen in Oxyde appear in the recent documents (Win+R, `shell:recent`).

## 2. Operations

Commit `ffaa7868`, with `4c5a586f` and `77309b0b`. Reference: [operations](../architecture/operations.md).

**Operations window**

- Copy `big.bin` and `many` from `src` to `dst`. The title bar button opens the window on the newest running operation; a compact notification opens it on its own.
- List on the left grouped Running and Finished with counts; the selected row in the selection fill. Drag the edge between list and detail; restart Oxyde: the width is kept; a double click on the edge restores it. The window restores its size and maximized state.
- Detail: kind, sources, state badge, volume and count, From and destination (a click opens them in the main window), progress bar and graph as wide as each other.
- Items table: badges per state, filter field (on its own line when the window is narrow), sort by Name, Type, Size, State; a double click shows the item selected in its folder in the main window.
- Footer: Pause and Resume (the bar, percentage and graph dim, "Paused" replaces speed and remaining time, the compact notification shows the pause icon), Turbo, Cancel, Remove finished, Close. The final duration and average speed leave out the time spent paused.
- Run an operation on many files with a conflict: the conflict window keeps a readable comparison whatever the list's width, card metadata wraps, a double click on the list's edge restores its width, the column separator is drawn as the panes'.

**Closing during an operation**

- During a copy, close the main window (button, Alt+F4, taskbar's Close window): a prompt asks. *Don’t quit*, Esc and the prompt's close keep Oxyde; *Cancel and quit* cancels, waits (the waiting message only past 800 ms) and closes; no `.oxyde-copy-*` folder stays in `dst`.
- Same with the operation paused.
- Secondary windows (Operations, Properties, Viewer, search) close just before the main one, not shown through its fade.
- **Setup**, when an update is offered: installing it during an operation is refused with a notification; it proceeds once the operation ends.

**Interrupted operations**

1. Start copying `big.bin` to `dst`; with Oxyde's hidden files shown, a `.oxyde-copy-…` folder is in `dst`, and a note in `%APPDATA%\Oxyde\interrupted\`.
2. Kill Oxyde and its workers mid-copy: `taskkill /F /T /IM oxyde.exe` (`Oxyde_FM.exe` for an installed copy).
3. Restart: a notification says a partial file was removed in `dst`; the folder and the note are gone.
4. Again, but add a file of your own in the staging folder before restarting: the folder is kept and the log says `Interrupted staging kept`.
5. Same with a compression (`.oxyde-compress-…`) and an undone move to another volume (`.oxyde-history-…`) if they last long enough to kill.

## 3. Windows' own windows

- **Inactive title bars** (`9e53b8e9`): with another application in front, the main window's and secondary windows' title text and caption symbols (minimize, maximize, close) turn to the secondary text color; back in front, they return.
- **Secondary title bars** (`f5dcd3de`): Operations, an operation's file list and Duplicates have the main title bar's square buttons, touching each other, 0.5 rem before the window controls.
- **Tooltips** (`792597c1`): a button's tooltip centered under it; near the bottom of the screen it goes above; at the window's edge it overflows the window; on the second screen at another scale it keeps its size and place. File, tree and sidebar hovers still follow the pointer. Menu frame, 13 px label, wrapping past 24 rem.
- **File drag ghost** (`25c583cd`): drag files inside Oxyde, to Explorer and to the desktop: the ghost has the lifted shadow, keeps its distance to the cursor, flips at the right and bottom of the work area without the shadow pushing it, on both screens. Ctrl and Shift still change the effect without moving the pointer.
- **Tab drag** (`0f624972`): a ghost at the tab's width, the other tabs and + sliding aside; with many tabs, the chevrons scroll the strip and dragging a tab or files near its ends scrolls it. Close a tab on release of its button or with the middle button.
- **Menus** (`c7af9deb`, `edc8bc5f`): a long menu's scrollbar thumb keeps following when dragged outside the menu, over the main window, and nothing in the main window lights up meanwhile; once the pointer leaves a menu, no row stays highlighted. Settings submenus open after resting 250 ms or on a click, beside their row, without title; the collapsed rail's pinned menu opens on hover and stays on the following click.
- **Prompts** (`5f6dfe0d`): rename, compression, network drive, Open With, trash settings, archive password: buttons press on release, Tab and Shift+Tab cycle through every stop, the accent ring shows only after a keyboard move, Enter in trash settings fields saves, refusals show under the field.

## 4. Shared interface, on Windows

These changes are drawn by the same code on both systems; check them once on Windows for scaling, fonts and native windows.

- **Bars and buttons** (`6ef7d9c3`, `8e1641eb`, `0fbce6bb`): title, tab, navigation and status bars at the same height; navigation, quick search, Viewer and status buttons as full-height square cells; collapsed sidebar as narrow as a bar; settings rows and menu entries at that height; separators spaced alike in menus, settings, properties, trash settings, network dialog and sidebar.
- **Lists and menus** (`7159849b`, `818e6877`): settings choices, permissions, shortcut run mode, drive letters, path completion, advanced search unit: the menus' surface, opening 0.25 rem under their button and as wide as it. The default archive format, settings picker rows and Tools no longer open their lists to the right of the button.
- **Fields** (`c1b41eaa`, `12bb41ee`, `dbf966d1`): 0.25 rem corners, in-field buttons (clear, submit path, calendar) as full-height cells, regex and show password kept visible; a refused path frames the field red; the quick search text starts at the separator; text action buttons rounded like fields.
- **Breadcrumb** (`e9dea011`, `3363b998`): `\` between crumbs, crumbs as high as the path field; hidden and system folders openable from a separator's list; a second press on a separator while a slow folder (network share) lists cancels it; parent crumbs brighten on hover.
- **Figures** (`8a15394a`): a `1` in file names is proportional; sizes, dates, counts, progress, media clocks, zoom and steppers stay aligned.
- **Controls** (`bc3727a5`, `e5c0f3ce`, `3de7406c`): square switches sliding with a spring, the trash window's toggled on release; keyboard focus rings on checkboxes and radios, partial checkboxes filled; steppers with Lucide minus and plus, repeating while held, the value restoring the default on click, Left and Right in a menu row; compression levels as segmented choices under an aligned format column, "Zip" spelled as elsewhere.
- **Scrollbars** (`c7af9deb`, `87e4d1e4`): thin centered thumbs widening on hover and drag, distinct colors for hover and drag; the wheel over a rail scrolls its content; a press in the track centers the thumb there, in lists, tree, menus and panels.
- **Theme editor** (`46626895`): six seeds on one line each, no scrolling; the picker's field and hue strip take arrows (Shift for 10%), Home and End; the hue reaches pure red at the right; an invalid code shows an error and is rewritten on leaving; Tab through name, actions, codes and picker.
- **Advanced search date picker** (`5f6dfe0d`): opens under its field as a popup, months by name, arrows and Page Up/Down, Enter or a click picks, a second press on the button closes.
- **Resize separators** (`4c5a586f`): 7 px grip on every separator (sidebar, split, Operations, conflict window, chooser), the accent after resting 250 ms, at once while dragging; the properties disk bar uses the shared bar.
- **Sidebar** (`8d6b0b30`, `edc8bc5f`, `0fbce6bb`): in the collapsed sidebar, only the deepest drive or Home holding the folder lights; reordering pinned folders slides the others aside; places, drives and the folder tree at their heights.
- **Duplicates** (`c156ba51`): group cards at one offset, folded or not, no notch between files.
- **Rename** (`76e57f5c`): rename `b.txt` to `z.txt` in a long folder sorted by name: it stays selected and scrolled into view.
- **Texts** (`5f6dfe0d`, `windows_onedrive`): no em dash left in the interface, empty values shown as `-`; OneDrive accounts read "OneDrive - Name" in the sidebar.
