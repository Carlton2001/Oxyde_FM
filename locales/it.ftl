common-cancel = Annulla

rename-title = Rinomina

rename-name-placeholder = Nome del file

rename-error-empty-name = Il nome non può essere vuoto.

rename-error-already-exists = Qui esiste già un file o una cartella con questo nome.

rename-error-not-found = L’elemento da rinominare non esiste più o non è accessibile.

rename-error-access-denied = Impossibile rinominare. Controlla i permessi di questo elemento.

rename-error-invalid-name = Rinomina non riuscita. Controlla il nome e riprova.

rename-error-unavailable = Questo elemento non può essere rinominato al momento.

rename-error-task-failed = Rinomina non riuscita. Riprova.

conflict-skip-all = Ignora tutti

conflict-replace-all = Sostituisci tutti

common-close = Chiudi

common-retry = Riprova

settings-title = Preferenze

settings-appearance = Aspetto

settings-theme = Temi
theme-window-title = Temi
theme-color-vision = Visione dei colori
theme-color-vision-description =
    Adatta i colori delle icone dei file e degli stati, in tutti i temi.
    Rosso-verde: protanomalia e deuteranomalia, in cui rosso, arancione e verde si confondono.
    Blu-giallo: tritanomalia, in cui blu e verde, giallo e viola si confondono.
color-vision-typical = Standard
color-vision-red-green = Rosso-verde
color-vision-blue-yellow = Blu-giallo
theme-builtin = Temi integrati
theme-custom = I miei temi
theme-custom-empty = Ancora nessun tema personalizzato.
theme-intro = Un tema di Oxyde si riduce a sei colori. Tutti gli altri ne derivano: testo secondario, bordi, stati al passaggio del puntatore, barre di scorrimento e i contrasti minimi che mantengono leggibile l’interfaccia.
theme-create-hint = Per creare il tuo tema, parti da quello più simile: sceglilo nell’elenco, poi «Crea da questo tema».
theme-create = Crea da questo tema
theme-delete = Elimina
theme-save = Salva
theme-name = Nome
theme-new-name = { $name } personalizzato
theme-color-background = Sfondo
theme-color-background-description = Pannello file inattivo, quando la vista è divisa.
theme-color-active-background = Sfondo attivo
theme-color-active-background-description = Pannello file attivo e finestre secondarie.
theme-color-sidebar = Barra laterale
theme-color-sidebar-description = Barra laterale, barra del titolo e barra delle schede.
theme-color-surface = Superficie
theme-color-surface-description = Menu, campi, intestazioni delle finestre e scheda attiva.
theme-color-text = Testo
theme-color-text-description = Testo principale; testo secondario e bordi ne derivano.
theme-color-accent = Accento
theme-color-accent-description = Selezione, pulsanti principali, focus e cartelle.
theme-picker-hint = Scegli un colore a sinistra, poi regolalo nel quadrato e nella striscia delle tinte, oppure digita il suo codice esadecimale (#3d5af1 o #fa0).
theme-contrast-low = Contrasto del testo: { $ratio }:1. Punta ad almeno 4,5:1 perché resti leggibile.

settings-scale = Scala dell’interfaccia

settings-scale-value = { $percentage }%

settings-scale-reset = Ripristina la scala al 100%

settings-language = Lingua

theme-dark = { $name } Scuro

theme-light = { $name } Chiaro

settings-files = File

settings-hidden = File nascosti

settings-hidden-description = Mostra file e cartelle nascosti, inclusi i nomi che iniziano con un punto

settings-system = File di sistema

settings-system-description = Mostra file e cartelle contrassegnati come elementi di sistema dal SO

settings-previews = Anteprime multimediali

settings-previews-description = Mostra le anteprime dei file nei suggerimenti dell’elenco

settings-grid-thumbnails = Miniature delle immagini nella griglia

settings-grid-thumbnails-description = Mostra le miniature delle immagini direttamente nelle schede della griglia

settings-about = Informazioni

settings-version = Versione { $version }


settings-save-error = Preferenze applicate, ma impossibile salvarle. Riprova.

settings-load-error = Impossibile leggere le preferenze salvate. Le modifiche non verranno salvate per proteggere il file.

status-folders = { $count ->
    [one] { $number } cartella
   *[other] { $number } cartelle
    }

status-files = { $count ->
    [one] { $number } file
   *[other] { $number } file
    }

view-details = Vista dettagli

view-grid = Vista griglia

selected-files = { $count ->
    [one] { $number } file selezionato ({ $size })
   *[other] { $number } file selezionati ({ $size })
    }

selected-folders = { $count ->
    [one] { $number } cartella selezionata ({ $size })
   *[other] { $number } cartelle selezionate ({ $size })
    }

selected-mixed = { $folder_count ->
    [one] { $folders } cartella
   *[other] { $folders } cartelle
    } e { $file_count ->
    [one] { $files } file
   *[other] { $files } file
    } selezionati ({ $size })

tooltip-type = Tipo: { $value }

tooltip-size = Dimensione: { $value }

tooltip-modified = Modificato: { $value }
tooltip-permissions = Permessi: { $value }

tooltip-location = Posizione: { $value }
tooltip-original-location = Posizione originale: { $value }
tooltip-deleted = Eliminato: { $value }

menu-open = Apri

menu-open-new-tab = Apri in nuova scheda
menu-open-terminal = Apri nella console

menu-pin-quick-access = Aggiungi ad Accesso rapido

menu-unpin-quick-access = Rimuovi da Accesso rapido

menu-cut = Taglia

menu-copy = Copia

menu-paste = Incolla

menu-rename = Rinomina

menu-delete = Elimina

menu-properties = Proprietà

menu-more-options = Altre opzioni

menu-new-folder = Nuova cartella
menu-new-file = Nuovo file

menu-duplicate-tab = Duplica scheda

menu-close-other-tabs = Chiudi altre schede

menu-close-tab = Chiudi scheda

menu-select-all = Seleziona tutto
menu-deselect-all = Deseleziona tutto

menu-select-all-checks = Seleziona tutte le caselle

menu-deselect-all-checks = Deseleziona tutte le caselle

column-name = Nome

column-type = Tipo

column-size = Dimensione

column-modified = Modificato
column-permissions = Permessi

file-type-folder = Cartella

file-type-file = File

file-type-symlink = Link simbolico
file-type-broken-link = Link interrotto

file-type-junction = Giunzione di directory

empty-filtered = Nessun elemento corrisponde ai filtri.

folder-loading = Caricamento della cartella…

folder-empty = Questa cartella è vuota.

preview-loading = Caricamento dell’anteprima…

operation-failed = Operazione non riuscita

path-placeholder = Percorso

navigation-back = Indietro

navigation-forward = Avanti

navigation-refresh = Aggiorna

navigation-undo = Annulla

navigation-redo = Ripristina

operation-copy = Copia

operation-move = Sposta

operation-rename = Rinomina

operation-delete = Elimina

operation-compress = Compressione

operation-extract = Estrazione

operation-extract-here = Estrai archivio

operation-new-folder = Nuova cartella


operations-title = { $count ->
    [1] Operazione ({ $count })
   *[other] Operazioni ({ $count })
    }

operations-show-panel = Mostra operazioni


operations-queued = In coda

operations-calculating = Calcolo in corso...

operations-running = In corso

operations-paused = In pausa

operations-cancelled = Annullata

operations-completed = Completato

operations-error = Non riuscita


operations-to = A: { $path }
operations-from = Da: { $path }
operations-source-summary = { $count ->
    [1] { $name } e { $number } altro
   *[other] { $name } e altri { $number }
    }
operations-group-running = In corso
operations-group-finished = Terminate
operations-dismiss-finished = Rimuovi terminate
operations-turbo = Turbo
operations-outcome-count = { $value }: { $number }

operations-items = { $count ->
    [1] { $done } / { $total } elemento
   *[other] { $done } / { $total } elementi
    }

operations-remaining = Tempo rimanente: { $seconds } s

operations-duration = Durata: { $duration }

operations-average = Media: { $speed }

operations-speed-waiting = In attesa...
operations-finalizing = Finalizzazione…

operations-pause = Pausa

operations-resume = Riprendi

operations-stop = Annulla operazione

operations-fast = Modalità rapida attiva. Fai clic per passare alla modalità normale

operations-normal = Modalità normale attiva. Fai clic per passare alla modalità rapida

operations-dismiss = Rimuovi


operations-empty = Nessuna operazione

name-filter-placeholder = Filtra per nome

undo-operation = Annulla: { $operation }

redo-operation = Ripeti: { $operation }



sidebar-quick-access = Accesso rapido

sidebar-drives = Unità

sidebar-locations = POSIZIONI

sidebar-network = Rete
sidebar-linux = Linux

# Platform name of the trash; `-trash` is aliased at load time.
-trash-windows = Cestino
-trash-linux = Cestino

sidebar-trash = { -trash }
sidebar-home = Cartella home

size-tiny = Minuscolo (< 10 KB)

size-small = Piccolo (10 KB - 1 MB)

size-medium = Medio (1 MB - 100 MB)

size-large = Grande (100 MB - 1 GB)

size-huge = Enorme (> 1 GB)

date-today = Oggi

date-yesterday = Ieri

date-this-week = Questa settimana

date-last-week = Settimana scorsa

date-older = Più vecchio

path-error-not-found = Questa cartella non è stata trovata o non è più disponibile.

path-error-access-denied = Cartella protetta (Sono richiesti diritti di amministratore)

path-error-failed = Impossibile aprire la cartella. Controlla il percorso e riprova.

undo-failed = Impossibile annullare: { $detail }
undo-copy-identity-changed = Questo elemento non corrisponde più all’oggetto copiato originale: { $detail }. L’annullamento è stato interrotto per preservare i file.
undo-copy-unverified = La cronologia non consente di verificare l’identità di questa copia: { $detail }. I file sono stati conservati.

undo-overwrite-unavailable = Questa copia non può essere annullata perché ha sostituito file esistenti.

redo-failed = Impossibile ripetere: { $detail }
redo-copy-source-changed = L’origine è cambiata dopo la copia: { $detail }. La ripetizione è stata interrotta per preservare i file.
redo-copy-unverified = La cronologia non consente di verificare questa copia: { $detail }. Avvia una nuova copia dalla cartella di origine.

prepare-failed = Impossibile preparare l’operazione: { $detail }

transfer-failed = Trasferimento non riuscito: { $detail }

drag-copy = Copia

drag-move = Sposta

new-folder-name = Nuova cartella

new-folder-numbered = Nuova cartella ({ $number })
new-file-name = Nuovo file
new-file-numbered = Nuovo file ({ $number })

native-menu-empty = Nessun comando disponibile

native-menu-failed = Impossibile caricare il menu di sistema: { $detail }

drag-items = { $count ->
    [one] { $number } elemento
   *[other] { $number } elementi
    }

native-menu-loading = Caricamento del menu…

local-drive = Disco locale ({ $drive })
unlabeled-volume = Volume da { $size }

unknown-item = Sconosciuto

menu-expand-all = Espandi tutto

menu-collapse-all = Riduci tutto

menu-copy-path = Copia percorso

sidebar-collapse-all = Riduci tutto

sidebar-scroll-top = Torna su

sidebar-collapse = Comprimi barra laterale

sidebar-expand = Espandi

settings-windows-icons = Icone di Windows

settings-windows-icons-description = Usa le icone di Windows per file e cartelle nelle viste elenco e griglia.

settings-checkboxes = Caselle di selezione

settings-checkboxes-description = Seleziona più elementi senza tenere premuto Ctrl.

settings-permissions-column = Colonna Permessi
settings-permissions-column-description = Mostra i permessi di accesso in ottale nella vista elenco.

settings-path-tracking = Segui percorso attivo

settings-path-tracking-description = Espandi le cartelle superiori e mostra la cartella attiva nella barra laterale.

settings-date-format = Formato data

date-format-us = Stati Uniti (MM/GG/AAAA)

date-format-european = Europeo (GG/MM/AAAA)

date-format-iso = ISO 8601 (AAAA-MM-GG)

settings-network = Rete

settings-network-description = Mostra le risorse di rete nel pannello sinistro.
settings-system-integration = Integrazione di sistema
settings-default-file-manager = File manager predefinito
settings-default-file-manager-description = Quando un’altra applicazione apre una cartella, o propone «Mostra nella cartella», è Oxyde a mostrarla. L’icona del file manager nel dock apre sempre quello del sistema.
settings-default-file-manager-description-windows = Quando un’altra applicazione apre una cartella, o propone «Mostra nella cartella», è Oxyde a mostrarla, come con la scorciatoia Windows+E. Le applicazioni che avviano da sole Esplora file lo aprono sempre.
settings-default-file-manager-error = Impossibile cambiare il file manager predefinito.
shell-open-in-oxyde = Apri in Oxyde
taskbar-run-as-administrator = Esegui come amministratore
settings-file-chooser = Finestre Apri e Salva
settings-file-chooser-description = Quando un’altra applicazione chiede un file da aprire o un posto dove salvare, la finestra è quella di Oxyde. Le applicazioni che disegnano la propria finestra la mantengono.
settings-file-chooser-windows = Finestre Apri e Salva (sperimentale)
settings-file-chooser-description-windows = Quando un’altra applicazione chiede un file da aprire o un posto dove salvare, la finestra è quella di Oxyde. Le applicazioni che aggiungono proprie opzioni alla finestra, quelle avviate come amministratore e quelle a 32 bit mantengono quella di Windows.
settings-file-chooser-error = Impossibile cambiare le finestre Apri e Salva.
file-chooser-title = Apri file
file-chooser-folder-title = Seleziona cartella
file-chooser-select = Seleziona
file-chooser-save-title = Salva file
file-chooser-save-files-title = Salva i file
file-chooser-save = Salva
file-chooser-name = Nome
file-chooser-name-placeholder = Nome del file
file-chooser-replace-title = { $count ->
    [one] Sostituisci file
   *[other] Sostituisci i file
}
file-chooser-replace-prompt = { $count ->
    [one] Un file chiamato «{ $name }» esiste già. Vuoi sostituirlo?
   *[other] { $number } file esistono già. Vuoi sostituirli?
}
file-chooser-replace-location = { $count ->
    [one] Si trova in «{ $folder }».
   *[other] Si trovano in «{ $folder }».
}
file-chooser-replace-warning = { $count ->
    [one] Sostituirlo ne sovrascrive il contenuto.
   *[other] Sostituirli ne sovrascrive il contenuto.
}

settings-region-language = Regione e Lingua

settings-performance = Prestazioni

settings-turbo = Modalità Turbo

settings-search-limit = Limite ricerca

settings-data = Dati dell’applicazione

settings-clear-cache = Svuota cache app

settings-reset-preferences = Ripristina preferenze

title-new-panel = Nuovo pannello a destra

title-tools = Ricerca

tool-sizes = Calcola dimensioni

tool-search = Ricerca avanzata

quick-search = Ricerca rapida

quick-search-title = Cerca «{ $query }» in { $folder }

quick-search-results = Risultati della ricerca

quick-search-running = Ricerca in corso…

quick-search-limited = Limite dei risultati raggiunto

quick-search-empty = Nessun risultato

quick-search-failed = Impossibile cercare

quick-search-clear = Cancella

quick-search-stop = Interrompi ricerca

open-file-location = Apri percorso file

column-location = Posizione
column-state = Stato

tool-duplicates = Trova duplicati

tabs-new-tab = Nuova scheda

date-this-month = Questo mese



date-group-count = { $count ->
    [one] { $number } elemento
   *[other] { $number } elementi
    }

menu-sort-by = Ordina per

menu-sort-ascending = Crescente

menu-sort-descending = Decrescente

menu-sort-extension = Estensione

drive-eject = Espelli
drive-unmount = Smonta

map-network-drive = Connetti unità di rete

disconnect-network-drive = Scollega unità di rete

disconnect-this-network-drive = Disconnetti questa unità di rete

network-force-prompt = Ci sono file aperti o operazioni in corso su { $drive }. Forzare la disconnessione può interrompere una copia o causare la perdita delle modifiche non salvate. Vuoi continuare?

network-force-action = Forza disconnessione

network-category = Categoria

tooltip-network-category = Categoria: { $value }

tooltip-network-workgroup = Gruppo di lavoro: { $value }

tooltip-network-location = Percorso di rete: { $value }

network-workgroup = Gruppo di lavoro

network-location = Percorso di rete

network-computer = Computer

network-other-devices = Altri dispositivi
network-service = Condivisione { $protocol }

network-drive-failed = Impossibile modificare la connessione di rete. { $detail }

open-item-failed = Impossibile aprire questo elemento. { $detail }

network-map-description = Quale cartella di rete vorresti connettere?

network-map-hint = Specifica la lettera dell’unità e la cartella da connettere.

network-disconnect-description = Seleziona le unità di rete da disconnettere, poi fai clic su Disconnetti.

network-letter = Unità

network-folder = Cartella

network-username = Nome utente

network-password = Password

network-reconnect = Riconnetti all'accesso

network-finish = Fine

network-disconnect = Scollega

network-no-drives = Nessuna unità di rete trovata.

network-no-letter = Nessuna lettera di unità disponibile.

network-invalid-path = Inserisci un percorso di rete valido, ad esempio \\server\condivisione.

network-connected = Unità di rete connessa con successo!

network-disconnected = Unità di rete disconnesse correttamente.

network-working = Operazione in corso…

network-loading = Caricamento delle unità…
remote-login-title = Autenticazione richiesta
remote-question-title = Conferma richiesta
forget-recent-server = Rimuovi dai server recenti
remote-domain = Dominio
remote-anonymous = Connetti in modo anonimo
remote-remember-never = Dimenticare la password immediatamente
remote-remember-session = Ricordare la password fino al termine della sessione
remote-remember-always = Ricordare per sempre
remote-retry = Credenziali rifiutate. Riprova.
remote-connect = Connetti
remote-connect-failed = Impossibile connettersi a questa posizione.
remote-connect-failed-toast = Connessione impossibile: { $detail }
archive-password-title = Archivio protetto
archive-password-prompt = «{ $name }» è protetto da password.
archive-password-wrong = Password errata. Riprova.
archive-password-unlock = Sblocca
archive-password-required = Questo archivio è protetto da password.
archive-password-rejected = La password di questo archivio è errata.
archive-protection-none = Nessuna password
archive-protection-password = Protetto da password
archive-password-confirm = Conferma
archive-password-empty = Inserisci una password.
archive-password-mismatch = Le password non corrispondono.
archive-password-too-long = La password è troppo lunga.
open-with = Apri con
open-with-other = Altra applicazione…
open-with-prompt = Scegli un'applicazione per aprire «{ $name }».
open-with-prompt-many = Scegli un'applicazione per aprire { $count } file.
open-with-always = Usa sempre per questo tipo di file
open-with-recommended = Applicazioni consigliate
open-with-related = Applicazioni correlate
open-with-other-applications = Altre applicazioni
open-with-none = Nessuna applicazione può aprire questi file.
open-with-loading = Caricamento delle applicazioni…
open-with-search = Cerca applicazioni…
open-with-default = Predefinita

drive-eject-failed = Impossibile espellere l’unità. Un’altra applicazione potrebbe usarla. { $detail }
drive-unmount-failed = Impossibile smontare il volume. Un’altra applicazione potrebbe usarlo. { $detail }
drive-holders = Aperto in: { $applications }.
drive-mount-failed = Impossibile montare il volume. { $detail }
drive-mount-hibernated = Windows non è stato arrestato completamente (ibernazione o avvio rapido), quindi questo volume resta bloccato. Arresta completamente Windows, quindi riprova.


compression-fast = Rapida

compression-normal = Bilanciata

compression-best = Massima

compress = Comprimi

compression = Compressione

compression-format = Formato

disk-image-mount = Monta

disk-image-unmount = Smonta

disk-image-mount-failed = Impossibile montare l’immagine. { $detail }

disk-image-unmount-failed = Impossibile smontare l’immagine. { $detail }

disk-image-open-failed = Impossibile aprire l’immagine. { $detail }

disk-image-busy = Un’operazione sui file sta ancora usando questa immagine.

disk-image-no-volume = Nessun volume accessibile è stato assegnato a questa immagine.

advanced-settings = Impostazioni

advanced-help = Aiuto

advanced-search-for = Cerca

advanced-search-for-placeholder = *.* o nomefile.txt

advanced-search-in = Cerca in

advanced-search-subfolders = Cerca nelle sottocartelle

advanced-search-in-archives = Cerca negli archivi

advanced-find-text = Trova testo nel file

advanced-find-text-placeholder = Cerca nei contenuti (non binari)...

advanced-global-options = Opzioni globali

advanced-case-sensitive = Distingui maiuscole

advanced-ignore-accents = Ignora accenti

advanced-reset = Ripristina

advanced-search = Cerca

advanced-regex-help-title = Modelli comuni

advanced-use-regex = Espressione regolare

advanced-regex-any-char = Qualsiasi carattere

advanced-regex-any-char-desc = Corrisponde a qualsiasi carattere singolo. Esempio: "b.t" trova "bat", "bit", "but".

advanced-regex-digit = Cifre

advanced-regex-digit-desc = Corrisponde a qualsiasi numero da 0 a 9.

advanced-regex-word-char = Caratteri alfanumerici

advanced-regex-word-char-desc = Corrisponde a lettere, numeri e trattini bassi.

advanced-regex-start-line = Inizio riga

advanced-regex-start-line-desc = Corrisponde solo all'inizio del nome.

advanced-regex-end-line = Fine riga

advanced-regex-end-line-desc = Corrisponde solo alla fine del nome.

advanced-regex-one-or-more = Uno o più

advanced-regex-one-or-more-desc = Corrisponde a una o più occorrenze del carattere precedente.

advanced-regex-optional = Opzionale

advanced-regex-optional-desc = Corrisponde a zero o una occorrenza del carattere precedente.

advanced-regex-either-or = O l'uno o l'altro

advanced-regex-either-or-desc = Corrisponde alla parte sinistra o destra.

advanced-regex-range = Intervallo caratteri

advanced-regex-range-desc = Corrisponde a qualsiasi carattere tra le parentesi quadre.

advanced-regex-negated-set = Insieme negato

advanced-regex-negated-set-desc = Corrisponde a qualsiasi carattere NON presente tra le parentesi.

advanced-regex-group = Gruppo

advanced-regex-group-desc = Raggruppa più simboli.

advanced-regex-count-exact = Conteggio esatto

advanced-regex-count-exact-desc = Corrisponde esattamente n volte.

advanced-size = Dimensione

advanced-date = Modificato

advanced-min = Min

advanced-max = Max

advanced-invalid = Controlla i modelli, le dimensioni e le date (AAAA-MM-GG).

advanced-date-placeholder = AAAA-MM-GG

advanced-weekdays = Lun Mar Mer Gio Ven Sab Dom
advanced-months = gennaio febbraio marzo aprile maggio giugno luglio agosto settembre ottobre novembre dicembre

duplicate-locations = Posizioni di ricerca

duplicate-settings = Impostazioni di ricerca

duplicate-name = Nome

duplicate-size = Dimensione

duplicate-content = Contenuto (lento)

duplicate-current = Cartella corrente

duplicate-warning = La selezione di più posizioni può rallentare notevolmente la ricerca.

duplicate-filter = Filtra risultati...

duplicate-idle = Configura la ricerca e fai clic su Cerca.

duplicate-empty = Nessun duplicato trovato.

duplicate-scanning = Analisi dei file: { $current }

duplicate-hashing = Confronto del contenuto: { $current } / { $total }

duplicate-groups = { $groups ->
    [one] { $group_count } gruppo di duplicati
   *[other] { $group_count } gruppi di duplicati
    }

duplicate-files = { $file_count ->
    [one] { $file_number } file
   *[other] { $file_number } file
    }

duplicate-group = Gruppo { $index }

duplicate-failed = Ricerca non riuscita: { $error }

duplicate-size-range = { $min } – { $max }

duplicate-disk = Disco n. { $disk }

duplicate-go-to-folder = Fai clic per andare alla cartella

folder-size-empty = Vuoto
folder-size-provisional-description =
    Dimensione da aggiornare.
    Il ricalcolo automatico attende una pausa nell’attività.
    Aggiornare per forzare l’aggiornamento.

quick-search-tooltip = Cerca nella cartella corrente

settings-clear-cache-confirm = Eliminare le cache di icone e miniature? Verranno ricreate quando necessario.

settings-reset-confirm = Ripristinare le preferenze predefinite? Le schede aperte e i tuoi temi personalizzati verranno mantenuti.

settings-cache-clearing = Svuotamento della cache…

settings-cache-cleared = Cache svuotata.

settings-cache-error = Impossibile svuotare l’intera cache. Riprova.

settings-preferences-reset = Preferenze ripristinate.

settings-turbo-description = Modalità Turbo predefinita per nuove copie, spostamenti, eliminazioni, spostamenti nel { -trash }, compressioni ed estrazioni. Si applica anche ai calcoli delle dimensioni delle cartelle, inclusi quelli in corso. Le operazioni sui file in corso mantengono la propria impostazione individuale.

settings-search-limit-description = Numero massimo di risultati per nuove ricerche rapide e avanzate. Incrementi di 500, minimo 500.

navigation-up = Su

about-repository = Repository GitHub
about-main-tools = Strumenti principali
update-available-toast = Oxyde { $detail } è disponibile. Installala da Preferenze › Informazioni.
update-failed-toast = Aggiornamento non riuscito: { $detail }
update-check = Cerca aggiornamenti
update-checking = Ricerca in corso…
update-current = Oxyde è aggiornato
update-check-failed = Ricerca non riuscita, riprova
update-install = Installa la versione { $version }
update-download-portable = Scarica la versione { $version }
update-downloading = Download dell’aggiornamento…

settings-search-limit-reset = Ripristina il limite predefinito di 3.000 risultati

menu-copy-name = Copia nome

trash-restore = Ripristina
trash-restore-all = Ripristina tutto
trash-delete-permanent = Elimina definitivamente
trash-delete-all = Svuota il { -trash }
trash-original-location = Posizione originale
trash-deleted-at = Data di eliminazione

trash-restore-selection = Ripristina la selezione

original-location-filter-placeholder = Filtra per posizione originale…

trash-properties = Proprietà del { -trash }
trash-locations = Posizione del { -trash }
trash-available-space = Spazio disponibile
trash-selected-settings = Impostazioni per la posizione selezionata
trash-custom-size = Dimensione personalizzata
trash-max-capacity = Dimensione massima
trash-dont-move = Non spostare i file nel { -trash }
trash-delete-immediately = Elimina i file immediatamente
trash-confirm-delete = Mostra conferma di eliminazione
trash-megabytes = MB
trash-settings-error = Impossibile applicare le impostazioni. Controlla la capacità.
trash-auto-delete = Eliminazione automatica
trash-auto-delete-old = Elimina automaticamente gli elementi vecchi
trash-retention-days = Conserva gli elementi (giorni)
trash-gnome-retention-note = Impostazione di GNOME, applicata qualunque sia l’applicazione che ha eliminato i file.
trash-kde-time-limit = Elimina gli elementi più vecchi di (giorni)
trash-kde-size-limit = Limita la dimensione (% del disco)
trash-kde-limit-action = Al raggiungimento del limite
trash-kde-warn = Avvisa
trash-kde-delete-oldest = Elimina i più vecchi
trash-kde-delete-largest = Elimina i più grandi
trash-kde-retention-note = Impostazioni di KDE, applicate ai file eliminati da Dolphin e dalle applicazioni KDE.
trash-retention-error = Impossibile applicare le impostazioni. Controlla i valori inseriti.

trash-settings-ok = OK

trash-item-count = { $count ->
    [one] ({ $formatted } elemento)
   *[other] ({ $formatted } elementi)
    }


drive-tooltip-usage = { $used } / { $total }

properties-system = Proprietà di sistema
properties-created = Creato
properties-accessed = Ultimo accesso
properties-link-target = Destinazione del link
properties-used = Spazio utilizzato
properties-capacity = Capacità
properties-contains = Contenuto
properties-counts = { $files } file, { $folders } cartelle
properties-drive = Unità
properties-file-system = File system

properties-copy-name-double-click = Doppio clic per copiare il nome

toast-copied = Copiato negli appunti
toast-cut = Tagliato negli appunti
toast-names-copied = { $count ->
    [one] { $number } nome copiato
   *[other] { $number } nomi copiati
    }
toast-paths-copied = { $count ->
    [one] { $number } percorso copiato
   *[other] { $number } percorsi copiati
    }
toast-restored = { $count ->
    [one] { $number } elemento ripristinato
   *[other] { $number } elementi ripristinati
    }
toast-purged = { $count ->
    [one] { $number } elemento eliminato definitivamente
   *[other] { $number } elementi eliminati definitivamente
    }
toast-mounted = Immagine disco montata
toast-unmounted = Immagine disco smontata

toast-undo = Operazione annullata
toast-redo = Operazione ripetuta

toast-failed = Operazione non riuscita: { $detail }

toast-copy-completed = Copia completata
toast-move-completed = Spostamento completato
toast-trash-completed = Spostato nel { -trash }
toast-delete-completed = Eliminazione definitiva completata
toast-compress-completed = Archivio creato
toast-extract-completed = Estrazione completata

conflict-source = Nuovo file
conflict-target = File esistente
conflict-newer = Più recente
conflict-older = Più vecchio
conflict-larger = Più grande
conflict-items = elementi
conflict-skip = Ignora
conflict-replace = Sostituisci
conflict-all = Tutti
conflict-identical = Identici
conflict-search = Filtra risultati…
conflict-empty = Nessun conflitto corrispondente
conflict-prompt = Un elemento chiamato «{ $name }» esiste già. Cosa vuoi fare?
conflict-advanced-title = Risoluzione dei conflitti
conflict-advanced = Avanzato
conflict-apply = Applica decisioni
conflict-step-title = { $current } / { $total } Conflitto di file
conflict-inherited = Ereditato da { $parent }: { $action }
conflict-back = Indietro
conflict-recent-tab = Recenti
conflict-older-tab = Vecchi
conflict-automatic = Auto
conflict-apply-summary = Applica: { $replace_count ->
        [one] { $replace_number } sostituzione
       *[other] { $replace_number } sostituzioni
    }, { $skip_count ->
        [one] { $skip_number } ignorato
       *[other] { $skip_number } ignorati
    }

peek-failed = Impossibile aprire Peek
properties-general = Generale
properties-shortcut = Collegamento
shortcut-target = Destinazione
shortcut-arguments = Argomenti
shortcut-working-directory = Avvia in
shortcut-comment = Commento
shortcut-run = Esegui
shortcut-normal = Finestra normale
shortcut-minimized = Ridotta a icona
shortcut-maximized = Ingrandita
shortcut-saving = Salvataggio…
shortcut-invalid-target = La destinazione non può essere vuota e i campi devono essere validi.

# Archive cache recovery and resource limits
archive-cache-full = Impossibile aprire { $detail }. Oxyde non può liberare abbastanza spazio: alcune copie sono ancora in uso, sono state modificate o sono appena state aperte. Chiudi i documenti inutilizzati e riprova, oppure estrai questo file in una cartella a tua scelta.
archive-cache-busy = Impossibile aprire { $detail }. È ancora in corso la preparazione dell’apertura di un altro archivio. Riprova tra poco, oppure estrai questo file in una cartella a tua scelta.
archive-cache-unavailable = Impossibile aprire { $detail }. Oxyde non ha potuto preparare la copia temporanea. Puoi estrarre questo file in una cartella a tua scelta.
archive-cache-extract = Estrai e apri…
archive-cache-exported = File estratto: { $detail }
archive-limit-bytes = Elaborazione interrotta: la dimensione totale decompressa supera il limite di { $limit } per questa azione. Non viene considerata la dimensione compressa dell’archivio.
archive-processing-failed = L’elaborazione dell’archivio non è riuscita. Il file potrebbe essere danneggiato o richiedere troppe risorse.
quit-operations-title = Uscire da Oxyde?
quit-operations-prompt = { $count ->
    [one] Un’operazione è in corso o in pausa. Uscire la annulla: gli elementi non ancora elaborati restano come sono.
   *[other] { $number } operazioni sono in corso o in pausa. Uscire le annulla: gli elementi non ancora elaborati restano come sono.
}
quit-operations-stay = Non uscire
quit-operations-confirm = Annulla ed esci
quit-operations-cancelling = Annullamento delle operazioni…
update-waits-for-operations = Completa o annulla le operazioni in corso prima di installare l’aggiornamento.
interrupted-output-removed = { $count ->
    [one] Rimosso il file parziale di un’operazione interrotta in { $detail }
   *[other] Rimossi { $number } file parziali di operazioni interrotte in { $detail }
}
archive-recovery-notice = File conservati
archive-recovery-completed = L’azione è terminata, ma alcuni file o cartelle sono stati conservati. Controlla questi percorsi prima di eliminarli: { $locations }
archive-recovery-stopped = L’azione non è stata completata. Alcuni file o cartelle sono stati conservati, inclusi risultati già pubblicati. Controlla questi percorsi prima di riprovare: { $locations }
archive-recovery-more = Altri { $count } percorsi sono riportati nel registro dell’applicazione.
archive-limit-source-bytes = Creazione dell’archivio interrotta: la dimensione totale dei file di origine supera il limite di { $limit } per questa azione.
archive-limit-encoded-bytes = Creazione dell’archivio interrotta: la dimensione di uscita supera il limite di { $limit } per questa azione.
archive-limit-entries = Elaborazione interrotta: l’archivio contiene più di { $limit } file e cartelle.
archive-limit-depth = Elaborazione interrotta: un percorso dell’archivio supera { $limit } livelli, incluso il nome del file.
archive-limit-time = Elaborazione interrotta: è stato raggiunto il tempo massimo di elaborazione attiva di { $limit } minuti. Le pause non vengono conteggiate.
archive-limit-decoded = Elaborazione interrotta: i dati decompressi, comprese le intestazioni, superano { $limit }.
archive-source-changed = L’archivio è cambiato durante l’operazione. Oxyde ha interrotto l’operazione e conservato la versione attuale dell’archivio. Ripeti l’operazione usando questa versione.

quick-search-archives-skipped =
    { $count ->
        [one] Risultati incompleti: { $number } archivio ignorato
       *[other] Risultati incompleti: { $number } archivi ignorati
    }
quick-search-archive-skipped-detail = Primo archivio ignorato: { $path }. { $reason }
quick-search-archive-unreadable = Questo archivio non è valido, non è supportato o non è leggibile.
quick-search-archive-content-not-searched = Il contenuto degli archivi non viene cercato. Le voci di questo archivio vengono escluse quando è attivo un filtro sul contenuto.
quick-search-archive-dates-unavailable = Le date delle voci non sono disponibili per questo formato di archivio. Le sue voci vengono escluse quando è attivo un filtro sulla data.

undo-new-folder-changed = La cartella non corrisponde più a quella creata: { $detail }. L’annullamento è stato interrotto per preservare i file.
undo-new-folder-unverified = La cronologia non consente di verificare questa cartella: { $detail }. La cartella e il suo contenuto sono conservati.
undo-new-folder-not-empty = La cartella ora contiene file o sottocartelle: { $detail }. L’annullamento non elimina il suo contenuto.
undo-new-folder-missing = La cartella creata non è stata trovata: { $detail }. L’annullamento è stato interrotto senza eliminare altri elementi.
redo-new-folder-conflict = Esiste già un elemento in questa posizione: { $detail }. Il ripristino non lo sostituisce.
redo-new-folder-parent-missing = La cartella superiore non è stata trovata: { $detail }. Il ripristino non ricrea le cartelle superiori.
redo-new-folder-unverified = La cronologia non consente di ripristinare la creazione di questa cartella: { $detail }. Non è stato creato alcun elemento.

redo-move-changed = Il file o la cartella è cambiato dopo l’operazione: { $detail }. Ripeti spostamento è stato interrotto per preservare i tuoi file.
redo-move-conflict = Un elemento occupa la destinazione: { $detail }. Ripeti spostamento non lo sostituisce.
redo-move-unverified = Questa cronologia non consente di ripetere lo spostamento in sicurezza: { $detail }. Avvia nuovamente lo spostamento dai file attuali.
redo-move-incomplete = Lo spostamento è stato interrotto e alcuni elementi non sono tornati nella posizione originale. File o cartelle sono stati conservati per il recupero: { $detail }. Controlla queste posizioni prima di riprovare.

undo-recovery-incomplete = L’annullamento è stato interrotto e alcuni elementi non sono tornati nella posizione originale. File o cartelle sono stati conservati per il recupero. Controlla queste posizioni prima di riprovare: { $detail }.
preview-unavailable = Impossibile caricare questa anteprima. Prova ad aprire il file direttamente.
preview-image-limit = Questa immagine supera i limiti dell’anteprima. Il file rimane accessibile.
preview-animation-static = Anteprima statica: l’animazione supera i limiti dell’anteprima.

undo-move-changed = Impossibile annullare questo spostamento: il file o la cartella è stato sostituito o ricreato. È stato conservato nella posizione attuale. { $detail }
undo-move-unverified = Impossibile annullare questo spostamento in sicurezza: non è stato possibile verificare l’identità del file o della cartella. L’elemento è stato conservato. { $detail }

undo-rename-changed = Impossibile annullare questa rinomina: il file o la cartella è stato sostituito. È stato conservato nella posizione attuale. { $detail }
undo-rename-unverified = Impossibile annullare questa rinomina in sicurezza: non è stato possibile verificare l’identità del file o della cartella. L’elemento è stato conservato. { $detail }
redo-rename-changed = Impossibile ripetere questa rinomina: il file o la cartella è stato sostituito. È stato conservato nella posizione attuale. { $detail }
redo-rename-unverified = Impossibile ripetere questa rinomina in sicurezza: non è stato possibile verificare l’identità del file o della cartella. L’elemento è stato conservato. { $detail }

transfer-source-changed = L’origine è stata sostituita o non è più un file o una cartella normale. L’operazione è stata interrotta; gli elementi già trasferiti vengono conservati. Controlla l’origine e riprova.
transfer-source-unverified = Non è possibile verificare l’identità dell’origine in questa posizione. L’operazione è stata interrotta; gli elementi già trasferiti vengono conservati. Prova da una cartella locale compatibile.

trash-restore-ambiguous = Più elementi del { -trash } provengono dalla stessa posizione. Apri il { -trash } e seleziona l’elemento preciso da ripristinare. Gli elementi già ripristinati vengono conservati.
invalid-encoding-name = { $name } (codifica non valida)
unrepresentable-path = Il nome di «{ $path }» non è un testo UTF-8 valido. Rinominalo prima di copiarlo, spostarlo, eliminarlo o comprimerlo; non è stato modificato nulla.
properties-permissions = Permessi
properties-contents = Contenuto
permissions-owner = Proprietario
permissions-group = Gruppo
permissions-others = Altri
permissions-access = Accesso
permissions-executable = Consentire l’esecuzione del file come programma
permissions-none = Nessuno
permissions-read-only = Sola lettura
permissions-read-write = Lettura e scrittura
permissions-list-files = Solo elencare i file
permissions-access-files = Accedere ai file
permissions-create-delete = Creare ed eliminare file
permissions-not-owner = Non sei il proprietario, quindi non puoi modificare questi permessi.
permissions-changed = L’elemento è cambiato dopo l’apertura di questa finestra; non è stato modificato nulla. Riapri le sue proprietà.
permissions-not-kept = Questo file system non conserva i permessi Unix (NTFS o FAT, per esempio): sono rimasti invariati.
permissions-unchanged = Invariato
permissions-enclosed = Modifica i permessi degli elementi contenuti
permissions-enclosed-hint = Si applica a tutti i file e le cartelle contenuti, a qualsiasi profondità, facendo clic su OK. I collegamenti simbolici e gli elementi su altri dischi non vengono modificati.
permissions-files = File
permissions-folders = Cartelle
permissions-incomplete = Non è stato possibile modificare { $count } elementi contenuti; gli altri e la cartella sì. Primo errore: { $detail }
permissions-multiple = Multipli
permissions-mixed = Misto
permissions-selection-incomplete = Non è stato possibile modificare { $count } elementi; gli altri sì. Primo errore: { $detail }
properties-section-details = Dettagli
properties-section-dates = Date
properties-section-space = Spazio
permissions-section-accounts = Proprietà
permissions-short-read = Lettura
permissions-short-read-write = Lettura-scrittura
permissions-short-list = Elencare
permissions-short-access = Accedere
permissions-short-modify = Modificare
trash-restore-changed = L’elemento del { -trash } è cambiato dopo la selezione. Il ripristino è stato interrotto. Aggiorna il { -trash } e seleziona nuovamente l’elemento. Gli elementi già ripristinati vengono conservati.
trash-restore-conflict = Un file o una cartella esiste già nella posizione di ripristino. Viene conservato e l’elemento interessato resta nel { -trash }. Rinomina o sposta la destinazione e riprova.
trash-restore-unavailable = Impossibile ripristinare questo elemento. Verifica che sia ancora nel { -trash } e che la cartella originale esista e sia accessibile, poi riprova. Gli elementi già ripristinati vengono conservati.

history-delete-changed = L’elemento non corrisponde più a quello registrato durante l’eliminazione o non è disponibile. Annulla o Ripeti è stato interrotto per preservare i file attuali. Gli elementi già elaborati vengono conservati.
history-delete-unverified = Oxyde non dispone delle informazioni necessarie per annullare o ripetere questa eliminazione in modo sicuro. L’azione è stata interrotta. Puoi controllare il { -trash } e ripristinare manualmente l’elemento desiderato.

preview-timed-out = Questa anteprima ha superato il limite di 15 secondi. Il file rimane accessibile.
copy-name-suffix = Copia
shell-delete-completed-error = Windows ha eliminato questo elemento, poi ha segnalato un errore. L’operazione completata rimane nella cronologia. Controlla la cartella e il { -trash } prima di riprovare.
shell-delete-unconfirmed = Windows non ha confermato l’eliminazione di questo elemento. L’eliminazione potrebbe essere stata parziale. Controlla la cartella e il { -trash } prima di riprovare.
shell-delete-in-use = Un file è in uso da un altro programma. Chiudilo, poi riprova.
shell-delete-access-denied = Accesso negato. Non hai i permessi per eliminare questo elemento.
delete-batch-failed = Non è stato possibile eliminare { $failed } elementi su { $total }.
delete-single-failed = Non è stato possibile eliminare l’elemento.
delete-batch-processed = Gli altri elementi sono stati eliminati. Gli elementi non riusciti sono segnalati nell’elenco dell’operazione.
delete-failure-cause = Causa
delete-retry-hint = Chiudi i programmi che usano questi file, poi fai clic su Riprova.
delete-failed-title = Eliminazione incompleta
transfer-failed-title = Operazione interrotta
transfer-failed-heading = L’operazione non è stata completata.
prepare-failed-title = Impossibile preparare
prepare-failed-heading = Non è stato possibile preparare gli elementi per l’operazione.
open-with-failed = Impossibile aprire
open-with-failed-heading = L’applicazione scelta non è riuscita ad aprire gli elementi.
operation-failed-heading = Non è stato possibile completare l’azione.
archive-recovery-completed-heading = L’azione è terminata, ma alcuni file o cartelle sono stati conservati.
archive-recovery-stopped-heading = L’azione non è stata completata. Alcuni file o cartelle sono stati conservati, inclusi risultati già pubblicati.
archive-recovery-completed-hint = Controlla questi percorsi prima di eliminarli.
archive-recovery-stopped-hint = Controlla questi percorsi prima di riprovare.
archive-recovery-locations = Percorsi conservati

archive-listing-metadata-limit = Impossibile mostrare il contenuto: l’elenco dei file supera { $limit }. Questo limite riguarda nomi e informazioni dei file, non le dimensioni decompresse. È possibile tentare l’estrazione con i limiti abituali.
archive-listing-worker-failed = Impossibile leggere il contenuto di questo archivio: il processo di lettura non è riuscito o la sua risposta non è valida. L’archivio non è stato modificato. È possibile riprovare o tentare l’estrazione con i limiti abituali.

toast-transfer-skipped = Tutti gli elementi sono stati ignorati.

properties-calculating-size = Calcolo della dimensione…
drag-new-tab = Crea una nuova scheda
drag-unavailable = Rilascio non disponibile
properties-loading = Caricamento delle proprietà…

storage-unavailable = Impossibile accedere ai dati di Oxyde. In modalità portatile, la cartella data deve essere scrivibile.

properties-selection-items = { $number } elementi
properties-total-size = Dimensione totale
properties-multiple-locations = Più posizioni
operations-affected-items = { $count ->
    [1] Elemento interessato ({ $number })
   *[other] Elementi interessati ({ $number })
    }
operations-items-filter = Cerca nomi o percorsi…
operations-no-matching-items = Nessun elemento corrispondente.
operations-items-loading = Caricamento degli elementi…
operations-item-done = Elaborato
operations-item-not-processed = Non elaborato
operations-item-skipped = Ignorato
operations-item-partial = Elaborato parzialmente
operations-item-failed = Non riuscito
operations-item-unknown = Stato sconosciuto
operations-item-interrupted = Interrotto

default-archive-format = Formato archivio predefinito

settings-onedrive = OneDrive
settings-onedrive-description = Mostra OneDrive nella barra laterale quando è configurato.

trash-restore-summary = Ripristino terminato: { $restored } elemento/i ripristinato/i, { $failed } non riuscito/i. Gli altri elementi sono stati elaborati. Consulta il registro per i dettagli.
transfer-destination-collision = Più elementi selezionati avrebbero la stessa destinazione: { $path }. Rinominali o trasferiscili separatamente. Nessun file è stato trasferito.
archive-link-refused = Un archivio non può contenere un link simbolico: { $path }. Oxyde rifiuta i link all’apertura e all’estrazione degli archivi, perché nessuna estrazione possa scrivere fuori dalla sua destinazione. Non è stato modificato nulla.

grouping = Raggruppa per
group-none = Nessuno
group-date = Data di modifica
group-type = Tipo di file
group-folders = Cartelle
group-archives = Archivi
group-applications = Applicazioni e programmi di installazione
group-documents = Documenti
group-images = Immagini
group-video = Video
group-audio = Audio
group-code = Codice e script
group-other = Altri
group-disk-images = Immagini disco
mouse-right-click = Clic destro
grouping-current = Raggruppato per: { $mode }
group-size = Dimensione
group-extension = Estensione
group-created = Data di creazione
group-deleted = Data di eliminazione
group-size-empty = Vuoto (0 byte)
group-no-extension = Senza estensione
group-origin-unknown = Posizione originale sconosciuta
group-date-unknown = Data sconosciuta
group-select-all = Seleziona tutti gli elementi del gruppo
group-deselect-all = Deseleziona tutti gli elementi del gruppo

viewer-title = Visualizzatore
viewer-visual-paused = Visualizzazione video in pausa
viewer-file-unavailable = Impossibile visualizzare questo file.
viewer-video-codec-missing = { $codec ->
    [unknown] Codec video mancante
   *[other] Codec video { $codec } mancante
}
viewer-audio-codec-missing = { $codec ->
    [unknown] Codec audio mancante
   *[other] Codec audio { $codec } mancante
}
viewer-codec-missing-hint = Installa il decoder GStreamer per questo formato, poi riavvia Oxyde.
viewer-tree-loading = Caricamento…
viewer-tree-empty = Cartella vuota
viewer-tree-failed = Impossibile leggere questo contenuto.
viewer-tree-locked = Il contenuto di questo archivio è protetto da password.
viewer-tree-password = Per aprire i suoi file è necessaria una password.
viewer-tree-more = { $count ->
    [one] + { $number } altro elemento
   *[other] + { $number } altri elementi
    }
viewer-tree-more-at-least = + { $number } o più elementi
viewer-tree-unsorted = Più di { $number } elementi: mostrati in ordine di lettura, senza ordinamento.
viewer-tree-row-limit = Troppe righe visualizzate: comprimi una cartella per espanderne un'altra.
viewer-tree-archive-ratio = compresso al { $percent } %
viewer-tree-counting = conteggio in corso…
viewer-tree-count-capped = conteggio interrotto a { $number } elementi
viewer-open = Apri il Visualizzatore
viewer-close = Chiudi il Visualizzatore
viewer-slide = Diapositiva
viewer-text-truncated = Vengono mostrati solo i primi 512 MiB del file.
markdown-note = Nota
markdown-tip = Suggerimento
markdown-important = Importante
markdown-warning = Avviso
markdown-caution = Attenzione
viewer-empty = Seleziona un file in un pannello esplora risorse.
viewer-detach = Scollega il Visualizzatore
viewer-detached = Il Visualizzatore è aperto in una finestra secondaria.
viewer-show-window = Mostra la finestra del Visualizzatore
viewer-play = Riproduci
viewer-pause = Pausa

viewer-rendered = Resa
viewer-code = Codice
viewer-highlighted = Evidenziazione sintattica

viewer-enable-highlighting = Attiva evidenziazione sintattica
viewer-disable-highlighting = Disattiva evidenziazione sintattica

group-scripts = Script

viewer-loading = Caricamento del file…
viewer-autoplay = Riproduzione automatica
viewer-enable-autoplay = Attiva la riproduzione automatica dei prossimi file multimediali
viewer-disable-autoplay = Disattiva la riproduzione automatica dei prossimi file multimediali

audio-no-tags = Nessun tag disponibile
audio-album = Album
audio-track = Traccia
audio-year = Anno
audio-mono = Mono
audio-stereo = Stereo
audio-channels = { $count } canali
video-frame-rate = { $rate } fps
image-megapixels = { $value } MP
image-frames = { $count } fotogrammi


quick-search-content-skipped =
    { $count ->
        [one] Contenuto non verificato: { $number } file
       *[other] Contenuto non verificato: { $number } file
    }
quick-search-content-skipped-detail = Ricerca del contenuto incompleta per { $path }: documento illeggibile o limite di risorse raggiunto.
delete-prompt-count = { $count ->
    [one] Eliminare questo elemento?
   *[other] Eliminare questi { $number } elementi?
    }
delete-permanent-prompt-count = { $count ->
    [one] Eliminare definitivamente questo elemento?
   *[other] Eliminare definitivamente questi { $number } elementi?
    }
delete-more = { $count ->
    [one] e { $number } altro
   *[other] e altri { $number }
    }
delete-restore-hint = Potrai ripristinarli dal { -trash }.
delete-irreversible = Questa azione non può essere annullata.
rename-extension-changed = L’estensione passa da .{ $from } a .{ $to }.
rename-extension-removed = Il nome perde l’estensione .{ $from }.
compression-protection = Protezione
