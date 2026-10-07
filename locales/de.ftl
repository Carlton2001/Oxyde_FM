common-cancel = Abbrechen

rename-title = Umbenennen

rename-name-placeholder = Dateiname

rename-error-empty-name = Der Name darf nicht leer sein.

rename-error-already-exists = Hier existiert bereits eine Datei oder ein Ordner mit diesem Namen.

rename-error-not-found = Das umzubenennende Element existiert nicht mehr oder ist nicht mehr zugänglich.

rename-error-access-denied = Umbenennen verweigert. Prüfen Sie die Berechtigungen dieses Elements.

rename-error-invalid-name = Umbenennen fehlgeschlagen. Prüfen Sie den Namen und versuchen Sie es erneut.

rename-error-unavailable = Dieses Element kann derzeit nicht umbenannt werden.

rename-error-task-failed = Umbenennen fehlgeschlagen. Versuchen Sie es erneut.

conflict-skip-all = Alle überspringen

conflict-replace-all = Alle ersetzen

common-close = Schließen

common-retry = Erneut versuchen

settings-title = Einstellungen

settings-appearance = Erscheinungsbild

settings-theme = Design

settings-scale = Oberflächenskalierung

settings-scale-value = { $reference } px

settings-scale-reset = Referenz auf 16 px pro rem zurücksetzen

settings-language = Sprache

theme-dark = { $name } Dunkel

theme-light = { $name } Hell

settings-files = Dateien

settings-hidden = Versteckte Dateien

settings-hidden-description = Versteckte Dateien und Ordner anzeigen, auch Namen mit einem Punkt am Anfang

settings-system = Systemdateien

settings-system-description = Dateien und Ordner anzeigen, die das Betriebssystem als Systemelemente kennzeichnet

settings-previews = Medienvorschau

settings-previews-description = Dateivorschauen in den Tooltips der Liste anzeigen

settings-grid-thumbnails = Bildminiaturen im Raster

settings-grid-thumbnails-description = Bildminiaturen direkt auf den Rasterkarten anzeigen

settings-about = Über

settings-version = Version { $version }


settings-save-error = Einstellungen angewendet, aber nicht gespeichert. Versuchen Sie es erneut.

settings-load-error = Gespeicherte Einstellungen konnten nicht gelesen werden. Zum Schutz der Datei werden Änderungen nicht gespeichert.

status-folders = { $count ->
    [one] { $number } Ordner
   *[other] { $number } Ordner
    }

status-files = { $count ->
    [one] { $number } Datei
   *[other] { $number } Dateien
    }

view-details = Detailansicht

view-grid = Rasteransicht

selected-files = { $count ->
    [one] { $number } Datei ausgewählt ({ $size })
   *[other] { $number } Dateien ausgewählt ({ $size })
    }

selected-folders = { $count ->
    [one] { $number } Ordner ausgewählt ({ $size })
   *[other] { $number } Ordner ausgewählt ({ $size })
    }

selected-mixed = { $folder_count ->
    [one] { $folders } Ordner
   *[other] { $folders } Ordner
    } und { $file_count ->
    [one] { $files } Datei
   *[other] { $files } Dateien
    } ausgewählt ({ $size })

tooltip-type = Typ: { $value }

tooltip-size = Größe: { $value }

tooltip-modified = Geändert: { $value }

tooltip-location = Speicherort: { $value }

menu-open = Öffnen

menu-open-new-tab = In neuem Tab öffnen

menu-pin-quick-access = An Schnellzugriff anheften

menu-unpin-quick-access = Von Schnellzugriff lösen

menu-cut = Ausschneiden

menu-copy = Kopieren

menu-paste = Einfügen

menu-rename = Umbenennen

menu-delete = Löschen

menu-properties = Eigenschaften

menu-more-options = Weitere Optionen

menu-new-folder = Neuer Ordner

menu-duplicate-tab = Tab duplizieren

menu-close-other-tabs = Andere Tabs schließen

menu-close-tab = Tab schließen

menu-select-all = Alle auswählen

menu-select-all-checks = Alle markieren

menu-deselect-all-checks = Alle Markierungen entfernen

column-name = Name

column-type = Typ

column-size = Größe

column-modified = Geändert

file-type-folder = Ordner

file-type-file = Datei

file-type-symlink = Symbolischer Link

file-type-junction = Verzeichnisverknüpfung

empty-filtered = Keine Elemente entsprechen den Filtern.

folder-loading = Ordner wird geladen…

folder-empty = Dieser Ordner ist leer.

preview-loading = Vorschau wird geladen…

operation-failed = Vorgang fehlgeschlagen

path-placeholder = Pfad

navigation-back = Zurück

navigation-forward = Vorwärts

navigation-refresh = Aktualisieren

navigation-undo = Rückgängig

navigation-redo = Wiederholen

operation-copy = Kopieren

operation-move = Verschieben

operation-rename = Umbenennen

operation-delete = Löschen

operation-compress = Komprimierung

operation-extract = Entpacken

operation-extract-here = Archiv entpacken

operation-new-folder = Neuer Ordner

operation-restore = Wiederherstellen

operations-title = { $count ->
    [1] Vorgang
   *[other] Vorgänge
    }

operations-show-panel = Vorgänge anzeigen

operations-hide-panel = Vorgänge ausblenden

operations-queued = In Warteschlange

operations-calculating = Berechnung...

operations-running = Wird ausgeführt

operations-paused = Pausiert

operations-cancelled = Abgebrochen

operations-completed = Abgeschlossen

operations-error = Fehlgeschlagen

operations-from = Von: { $path }

operations-to = Nach: { $path }

operations-items = { $done } / { $total } Elemente

operations-remaining = Verbleibende Zeit: { $seconds } s

operations-duration = Dauer: { $duration }

operations-average = Durchschnitt: { $speed }

operations-speed-waiting = Warten...

operations-pause = Anhalten

operations-resume = Fortsetzen

operations-stop = Vorgang abbrechen

operations-fast = Schnellmodus aktiv — klicken, um zum Normalmodus zu wechseln

operations-normal = Normalmodus aktiv — klicken, um zum Schnellmodus zu wechseln

operations-dismiss = Entfernen

operations-dismiss-all = Alle abgeschlossenen Vorgänge entfernen

operations-empty = Keine Vorgänge

name-filter-placeholder = Nach Name filtern

undo-operation = Rückgängig: { $operation }

redo-operation = Wiederholen: { $operation }

shortcut-back = Alt+Links

shortcut-forward = Alt+Rechts

sidebar-quick-access = Schnellzugriff

sidebar-drives = Laufwerke

sidebar-locations = ORTE

sidebar-network = Netzwerk

sidebar-trash = Papierkorb

size-tiny = Winzig (< 10 KB)

size-small = Klein (10 KB - 1 MB)

size-medium = Mittel (1 MB - 100 MB)

size-large = Groß (100 MB - 1 GB)

size-huge = Riesig (> 1 GB)

date-today = Heute

date-yesterday = Gestern

date-this-week = Diese Woche

date-last-week = Letzte Woche

date-older = Älter

path-error-not-found = Dieser Ordner wurde nicht gefunden oder ist nicht mehr verfügbar.

path-error-access-denied = Zugriff auf diesen Ordner verweigert. Prüfen Sie Ihre Berechtigungen.

path-error-failed = Ordner konnte nicht geöffnet werden. Prüfen Sie den Pfad und versuchen Sie es erneut.

undo-failed = Rückgängig machen fehlgeschlagen: { $detail }

undo-overwrite-unavailable = Diese Kopie kann nicht rückgängig gemacht werden, da vorhandene Dateien ersetzt wurden.

redo-failed = Wiederholen fehlgeschlagen: { $detail }

prepare-failed = Vorgang konnte nicht vorbereitet werden: { $detail }

transfer-failed = Übertragung fehlgeschlagen: { $detail }

drag-copy = Kopieren

drag-move = Verschieben

new-folder-name = Neuer Ordner

new-folder-numbered = Neuer Ordner ({ $number })

native-menu-empty = Keine Befehle verfügbar

native-menu-failed = Systemmenü konnte nicht geladen werden: { $detail }

drag-items = { $count ->
    [one] { $number } Element
   *[other] { $number } Elemente
    }

native-menu-loading = Menü wird geladen…

local-drive = Lokaler Datenträger ({ $drive })

unknown-item = Unbekannt

menu-expand-all = Alle ausklappen

menu-collapse-all = Alle einklappen

menu-copy-path = Pfad kopieren

sidebar-collapse-all = Alle einklappen

sidebar-scroll-top = Nach oben

sidebar-collapse = Seitenleiste einklappen

sidebar-expand = Vergrößern

settings-windows-icons = Windows-Icons

settings-windows-icons-description = Windows-Symbole für Dateien und Ordner in Listen- und Rasteransichten verwenden.

settings-checkboxes = Auswahl-Checkboxen

settings-checkboxes-description = Mehrere Elemente auswählen, ohne Strg gedrückt zu halten.

settings-path-tracking = Aktivem Pfad folgen

settings-path-tracking-description = Übergeordnete Ordner aufklappen und den aktiven Ordner in der Seitenleiste anzeigen.

settings-date-format = Datumsformat

date-format-us = USA (MM/TT/JJJJ)

date-format-european = Europa (TT/MM/JJJJ)

date-format-iso = ISO 8601 (JJJJ-MM-TT)

settings-network = Netzwerk

settings-network-description = Netzwerkumgebung im linken Bereich anzeigen.

settings-region-language = Region & Sprache

settings-performance = Leistung

settings-turbo = Turbo-Modus

settings-search-limit = Suchlimit

settings-data = Anwendungsdaten

settings-clear-cache = App-Cache leeren

settings-reset-preferences = Optionen zurücksetzen

title-new-panel = Neuer Bereich rechts

title-tools = Werkzeuge

tool-sizes = Größen berechnen

tool-search = Erweiterte Suche

quick-search = Schnellsuche

quick-search-title = „{ $query }“ in { $folder } suchen

quick-search-results = Suchergebnisse

quick-search-running = Suche läuft…

quick-search-limited = Ergebnislimit erreicht

quick-search-empty = Keine Ergebnisse

quick-search-failed = Suche nicht möglich

quick-search-clear = Leeren

quick-search-stop = Suche beenden

open-file-location = Dateipfad öffnen

column-location = Ort

tool-duplicates = Duplikate finden

tabs-new-tab = Neuer Tab

date-this-month = Diesen Monat

settings-group-by-date = Nach Datum gruppieren

settings-group-by-date-description = Dateien im aktiven Tab nach Änderungsdatum gruppieren.

date-group-count = { $count ->
    [one] { $number } Element
   *[other] { $number } Elemente
    }

menu-sort-by = Sortieren nach

menu-sort-ascending = Aufsteigend

menu-sort-descending = Absteigend

menu-sort-extension = Erweiterung

drive-eject = Auswerfen

map-network-drive = Netzlaufwerk verbinden

disconnect-network-drive = Netzlaufwerk trennen

disconnect-this-network-drive = Dieses Netzlaufwerk trennen

network-force-prompt = Auf { $drive } sind Dateien geöffnet oder Vorgänge aktiv. Eine erzwungene Trennung kann Kopiervorgänge unterbrechen oder ungespeicherte Änderungen verlieren. Möchten Sie fortfahren?

network-force-action = Trennung erzwingen

network-category = Kategorie

tooltip-network-category = Kategorie: { $value }

tooltip-network-workgroup = Arbeitsgruppe: { $value }

tooltip-network-location = Netzwerkadresse: { $value }

network-workgroup = Arbeitsgruppe

network-location = Netzwerkadresse

network-computer = Computer

network-other-devices = Andere Geräte

network-drive-failed = Netzwerkverbindung konnte nicht geändert werden. { $detail }

open-item-failed = Dieses Element konnte nicht geöffnet werden. { $detail }

network-map-description = Welchen Netzwerkordner möchten Sie verbinden?

network-map-hint = Geben Sie den Laufwerksbuchstaben und den Ordner an, den Sie verbinden möchten.

network-disconnect-description = Wählen Sie die zu trennenden Netzlaufwerke aus und klicken Sie auf Trennen.

network-letter = Laufwerk

network-folder = Ordner

network-username = Benutzername

network-password = Passwort

network-reconnect = Bei Anmeldung wiederherstellen

network-finish = Fertigstellen

network-disconnect = Trennen

network-no-drives = Keine Netzlaufwerke gefunden.

network-no-letter = Kein Laufwerksbuchstabe verfügbar.

network-invalid-path = Geben Sie einen gültigen Netzwerkpfad ein, zum Beispiel \\server\freigabe.

network-connected = Netzlaufwerk erfolgreich verbunden!

network-disconnected = Netzlaufwerke erfolgreich getrennt.

network-working = Vorgang läuft…

network-loading = Laufwerke werden geladen…

drive-eject-failed = Laufwerk konnte nicht ausgeworfen werden. Eine andere Anwendung verwendet es möglicherweise. { $detail }

archive-format = Archiv .{ $format }

compression-fast = Schnell

compression-normal = Ausgewogen

compression-best = Maximal

compress = Komprimieren

compression = Komprimierung

compression-format = Format

disk-image-mount = Einbinden

disk-image-unmount = Aushängen

disk-image-mount-failed = Abbild konnte nicht eingebunden werden. { $detail }

disk-image-unmount-failed = Abbild konnte nicht ausgehängt werden. { $detail }

disk-image-open-failed = Abbild konnte nicht geöffnet werden. { $detail }

disk-image-busy = Ein Dateivorgang verwendet dieses Abbild noch.

disk-image-no-volume = Diesem Abbild wurde kein zugängliches Volume zugewiesen.

advanced-settings = Einstellungen

advanced-help = Hilfe

advanced-search-for = Suchen nach

advanced-search-for-placeholder = *.* oder dateiname.txt

advanced-search-in = Suchen in

advanced-search-subfolders = Unterordner durchsuchen

advanced-search-in-archives = In Archiven suchen

advanced-find-text = Text in Datei finden

advanced-find-text-placeholder = In Inhalten suchen (nicht binär)...

advanced-global-options = Globale Optionen

advanced-case-sensitive = Groß-/Kleinschreibung

advanced-ignore-accents = Akzente ignorieren

advanced-reset = Zurücksetzen

advanced-search = Suchen

advanced-regex-help-title = Gängige Muster

advanced-use-regex = Regulärer Ausdruck

advanced-regex-any-char = Beliebiges Zeichen

advanced-regex-any-char-desc = Entspricht einem beliebigen Zeichen. Beispiel: "b.t" findet "bat", "bit", "but".

advanced-regex-digit = Ziffern

advanced-regex-digit-desc = Entspricht einer Zahl von 0 bis 9.

advanced-regex-word-char = Wortzeichen

advanced-regex-word-char-desc = Entspricht Buchstaben, Zahlen und Unterstrichen.

advanced-regex-start-line = Zeilenanfang

advanced-regex-start-line-desc = Entspricht dem Anfang des Namens.

advanced-regex-end-line = Zeilenende

advanced-regex-end-line-desc = Entspricht dem Ende des Namens.

advanced-regex-one-or-more = Eines oder mehrere

advanced-regex-one-or-more-desc = Entspricht einem oder mehreren des vorherigen Zeichens.

advanced-regex-optional = Optional

advanced-regex-optional-desc = Entspricht null oder einem des vorherigen Zeichens.

advanced-regex-either-or = Entweder / Oder

advanced-regex-either-or-desc = Entspricht dem linken oder rechten Teil.

advanced-regex-range = Zeichenbereich

advanced-regex-range-desc = Entspricht einem Zeichen in den eckigen Klammern.

advanced-regex-negated-set = Negierte Menge

advanced-regex-negated-set-desc = Entspricht jedem Zeichen, das NICHT in den Klammern steht.

advanced-regex-group = Gruppe

advanced-regex-group-desc = Gruppiert mehrere Symbole.

advanced-regex-count-exact = Genaue Anzahl

advanced-regex-count-exact-desc = Entspricht exakt n-mal.

advanced-size = Größe

advanced-date = Geändert

advanced-min = Min

advanced-max = Max

advanced-invalid = Prüfen Sie Muster, Größen und Daten (JJJJ-MM-TT).

advanced-date-placeholder = JJJJ-MM-TT

advanced-weekdays = Mo Di Mi Do Fr Sa So

duplicate-locations = Suchorte

duplicate-settings = Sucheinstellungen

duplicate-name = Name

duplicate-size = Größe

duplicate-content = Inhalt (langsam)

duplicate-current = Aktueller Ordner

duplicate-warning = Die Auswahl mehrerer Orte kann die Suche erheblich verlangsamen.

duplicate-filter = Ergebnisse filtern...

duplicate-idle = Konfigurieren Sie die Suche und klicken Sie auf Suchen.

duplicate-empty = Keine Duplikate gefunden.

duplicate-scanning = Dateien werden durchsucht: { $current }

duplicate-hashing = Inhalte werden verglichen: { $current } / { $total }

duplicate-groups = { $groups ->
    [one] { $group_count } Duplikatgruppe
   *[other] { $group_count } Duplikatgruppen
    }

duplicate-files = { $file_count ->
    [one] { $file_number } Datei
   *[other] { $file_number } Dateien
    }

duplicate-group = Gruppe { $index }

duplicate-failed = Suche fehlgeschlagen: { $error }

duplicate-more = Mehr laden

duplicate-disk = Datenträger Nr. { $disk }

duplicate-go-to-folder = Klicken, um zum Ordner zu wechseln

folder-size-empty = Leer

quick-search-current-folder = Aktueller Ordner

settings-clear-cache-confirm = Symbol- und Miniaturbild-Caches löschen? Sie werden bei Bedarf neu erstellt.

settings-reset-confirm = Standardeinstellungen wiederherstellen? Geöffnete Tabs bleiben erhalten.

settings-cache-clearing = Cache wird geleert…

settings-cache-cleared = Cache geleert.

settings-cache-error = Der Cache konnte nicht vollständig geleert werden. Versuchen Sie es erneut.

settings-preferences-reset = Einstellungen zurückgesetzt.

settings-turbo-description = Standard-Turbomodus für neue Kopier- und Verschiebevorgänge. Laufende Übertragungen behalten ihre Einstellung.

settings-search-limit-description = Maximale Ergebnisse für neue Schnell- und erweiterte Suchen. Schritte von 500, mindestens 500.

navigation-up = Nach oben

about-releases = Versionen ansehen
about-repository = GitHub-Repository
about-main-tools = Wichtigste Werkzeuge

settings-search-limit-reset = Auf das Standardlimit von 3.000 Ergebnissen zurücksetzen

menu-copy-name = Name kopieren

trash-restore = Wiederherstellen
trash-restore-all = Alles wiederherstellen
trash-delete-permanent = Endgültig löschen
trash-delete-all = Papierkorb leeren
trash-all = Alle
trash-original-location = Ursprünglicher Speicherort
trash-deleted-at = Löschdatum

trash-restore-selection = Auswahl wiederherstellen

original-location-filter-placeholder = Nach ursprünglichem Speicherort filtern…

trash-properties = Papierkorb-Eigenschaften
trash-locations = Papierkorb-Speicherort
trash-available-space = Verfügbarer Speicher
trash-selected-settings = Einstellungen für den ausgewählten Speicherort
trash-custom-size = Benutzerdefinierte Größe
trash-max-capacity = Maximale Größe
trash-dont-move = Dateien nicht in den Papierkorb verschieben
trash-delete-immediately = Dateien beim Löschen sofort entfernen
trash-confirm-delete = Löschbestätigung anzeigen
trash-megabytes = MB
trash-settings-error = Einstellungen konnten nicht angewendet werden. Prüfen Sie die Kapazität.

trash-settings-ok = OK
trash-delete-prompt = Ausgewählte Elemente löschen?

trash-item-count = { $count ->
    [one] ({ $formatted } Element)
   *[other] ({ $formatted } Elemente)
    }

trash-permanent-delete-prompt = Ausgewählte Elemente endgültig löschen?

drive-tooltip-usage = { $used } / { $total }

properties-system = Systemeigenschaften
properties-created = Erstellt
properties-accessed = Zugriff
properties-used = Belegter Speicher
properties-capacity = Kapazität
properties-contains = Inhalt
properties-counts = { $files } Dateien, { $folders } Ordner
properties-drive = Laufwerk

properties-copy-name-double-click = Doppelklicken, um den Namen zu kopieren

toast-copied = In die Zwischenablage kopiert
toast-cut = In die Zwischenablage ausgeschnitten
toast-names-copied = { $count ->
    [one] { $number } Name kopiert
   *[other] { $number } Namen kopiert
    }
toast-paths-copied = { $count ->
    [one] { $number } Pfad kopiert
   *[other] { $number } Pfade kopiert
    }
toast-restored = { $count ->
    [one] { $number } Element wiederhergestellt
   *[other] { $number } Elemente wiederhergestellt
    }
toast-purged = { $count ->
    [one] { $number } Element endgültig gelöscht
   *[other] { $number } Elemente endgültig gelöscht
    }
toast-mounted = Datenträgerabbild eingebunden
toast-unmounted = Datenträgerabbild ausgehängt

toast-undo = Vorgang rückgängig gemacht
toast-redo = Vorgang wiederholt

toast-failed = Vorgang fehlgeschlagen: { $detail }

toast-copy-completed = Kopieren abgeschlossen
toast-move-completed = Verschieben abgeschlossen
toast-trash-completed = In den Papierkorb verschoben
toast-delete-completed = Endgültiges Löschen abgeschlossen
toast-compress-completed = Archiv erstellt
toast-extract-completed = Entpacken abgeschlossen

conflict-source = Neue Datei
conflict-target = Vorhandene Datei
conflict-newer = Neuer
conflict-older = Älter
conflict-larger = Größer
conflict-items = Elemente
conflict-skip = Überspringen
conflict-replace = Ersetzen
conflict-all = Alle
conflict-identical = Identisch
conflict-search = Ergebnisse filtern…
conflict-empty = Keine passenden Konflikte
conflict-prompt = Ein Element namens „{ $name }“ existiert bereits. Was möchten Sie tun?
conflict-advanced-title = Konfliktlösung
conflict-advanced = Erweitert
conflict-apply = Entscheidungen anwenden
conflict-step-title = { $current } / { $total } Dateikonflikt
conflict-inherited = Von { $parent } übernommen: { $action }
conflict-back = Zurück
conflict-recent-tab = Neuer
conflict-older-tab = Älter
conflict-automatic = Auto
conflict-apply-summary = Anwenden: { $replace_count ->
        [one] { $replace_number } Ersetzung
       *[other] { $replace_number } Ersetzungen
    }, { $skip_count ->
        [one] { $skip_number } übersprungen
       *[other] { $skip_number } übersprungen
    }

peek-open = Mit PowerToys Peek öffnen
peek-failed = Peek konnte nicht geöffnet werden
properties-general = Allgemein
properties-shortcut = Verknüpfung
shortcut-target = Ziel
shortcut-arguments = Argumente
shortcut-working-directory = Ausführen in
shortcut-comment = Kommentar
shortcut-run = Ausführen
shortcut-normal = Normales Fenster
shortcut-minimized = Minimiert
shortcut-maximized = Maximiert
shortcut-saving = Wird gespeichert…
shortcut-invalid-target = Das Ziel darf nicht leer sein und die Felder müssen gültig sein.
