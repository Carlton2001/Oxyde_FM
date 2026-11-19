# Audit DIALOG-01

Fichiers lus en entier :

- [x] `shell/oxyde-file-dialog/examples/fake_launcher.rs`
- [x] `shell/oxyde-file-dialog/examples/show_dialog.rs`
- [x] `shell/oxyde-file-dialog/src/ask.rs`
- [x] `shell/oxyde-file-dialog/src/controls.rs`
- [x] `shell/oxyde-file-dialog/src/dialog.rs`
- [x] `shell/oxyde-file-dialog/src/items.rs`
- [x] `shell/oxyde-file-dialog/src/lib.rs`
- [x] `shell/oxyde-file-dialog/src/request.rs`

Résumé : 1 élevé, 2 moyens, 11 faibles, 1 optimisation. Aucun panic atteignable trouvé hors de `Show` (seul point protégé par `catch_unwind`) ; le risque principal est un blocage du thread UI de l'application hôte.

## Constats

### ask.rs:54-59, 68-69, 238 — élevé — plausible
Les deux pipes sont créés héritables, puis seul le bout gardé perd l'héritage, et les bouts du lanceur restent héritables jusqu'à la fin de `launch` : tout `CreateProcess` avec héritage lancé au même moment par un autre thread de l'application hôte (cas courant, sans liste de handles) duplique ces handles.
Scénario : l'hôte lance un processus en arrière-plan pendant l'ouverture du dialogue ; ce processus garde le bout écriture de la sortie, `read_all` n'atteint jamais EOF et `reader.join()` bloque le thread UI de l'hôte sans pomper de messages tant que ce processus vit (si c'est le bout écriture de l'entrée qui fuit, le lanceur n'atteint pas EOF sur stdin et la fenêtre propriétaire reste désactivée). La jointure n'a ni délai ni abandon possible.

### dialog.rs:145-146 — moyen — plausible
Le résultat de `GetFileName` du dialogue système est converti sans vérifier que le pointeur est non nul, alors que `controls.rs:190` fait cette vérification pour `GetEditBoxText`.
Scénario : sur une build où `GetFileName` renvoie S_OK avec un pointeur nul quand aucun nom n'est saisi (Open sans `SetFileName`), la lecture du pointeur nul provoque une violation d'accès, que `catch_unwind` n'attrape pas, et l'application hôte plante.

### dialog.rs:224-227, 263 — moyen — plausible
Seul `OnFileOk` est relayé aux écouteurs. `OnOverwrite` et `OnShareViolation` ne sont jamais appelés quand Oxyde répond.
Scénario : une application qui refuse l'écrasement de certains fichiers via `OnOverwrite` (FDEOR_REFUSE, par exemple le document source ou un fichier protégé) reçoit tout de même ce chemin et l'écrase.

### dialog.rs:239-241 — faible — confirmé
La requête est construite une seule fois, avant la boucle des tentatives.
Scénario : après un refus dans `OnFileOk`, Oxyde revient au dossier, au nom, au filtre et aux contrôles d'origine. Les choix précédents de l'utilisateur et les corrections faites par l'application (`SetFileName`, état des contrôles) sont perdus à la tentative suivante.

### dialog.rs:242 — faible — confirmé
`closed` est remis à zéro au début de chaque tentative.
Scénario : une application qui appelle `Close(hr)` depuis `OnFileOk` puis renvoie S_FALSE voit Oxyde rouvert au lieu d'une fermeture avec `hr`.

### dialog.rs:329-341 (et ask.rs:227-229) — faible — plausible
`Show` ne refuse pas un appel réentrant sur le même objet. Le second appel écrase `showing`, et le premier conclut ensuite à `Closed`.
Scénario : un minuteur de l'application rappelle `Show` pendant la boucle modale. Le premier `Show` renvoie « annulé » et le choix fait dans le premier Oxyde est perdu, alors que le dialogue système refuse un `Show` réentrant.

### dialog.rs:329-341 — faible — plausible
`show` ne prend pas de référence sur l'objet pendant la boucle modale, qui distribue les messages et les appels COM entrants.
Scénario : un `Release` final traité pendant `wait` (objet libéré par un gestionnaire de fermeture de fenêtre, ou par un appel inter-appartement marshalé) libère le `Dialog` encore utilisé par `show` : utilisation après libération dans le processus hôte.

### controls.rs:156-181, dialog.rs:213 — faible — plausible
Le code prévoit des builds où `GetControlState` du menu déroulant du bouton renvoie E_NOTIMPL, mais pas le même cas pour `GetControlItemState`, `GetSelectedControlItem` et `SetSelectedControlItem`.
Scénario : sur ces builds, tous les éléments sont filtrés et le menu disparaît sans bruit. Si le menu est quand même affiché, le choix « Ouvrir en lecture seule » n'est pas appliqué et l'application ouvre le fichier en écriture.

### controls.rs:136-140 — faible — confirmé
La recherche d'un texte qui nomme un contrôle remonte tous les contrôles du même groupe, sans exiger qu'ils soient adjacents. Pour les contrôles hors groupe, elle remonte toute la liste.
Scénario : un texte de note suivi de cases à cocher puis d'une liste déroulante sans étiquette devient l'étiquette de cette liste et disparaît des notes. Un même texte peut nommer plusieurs contrôles.

### dialog.rs:304-313 — faible — plausible
Les interfaces inconnues sont servies par le dialogue système, ce qui viole l'identité COM : depuis le pointeur renvoyé, la demande d'IUnknown ne rend pas l'objet enveloppant. Ce cas marque aussi le dialogue comme `special`.
Scénario : un appelant MTA ou un runtime qui interroge IMarshal, IExternalConnection ou IAgileObject peut obtenir l'interface interne. Le marshaling contourne alors l'objet enveloppant, ou bascule définitivement vers le dialogue système. Les comparaisons d'identité (ATL, RCW .NET) peuvent échouer.

### request.rs:175 (avec src/platform/windows_file_dialog.rs:143) — faible — confirmé
Côté Oxyde, les chemins non représentables en UTF-8 sont retirés de la réponse sans bruit. Une réponse « ok » sans chemin est lue comme une erreur, donc `Unavailable`.
Scénario : l'utilisateur choisit un fichier dont le nom NTFS contient une demi-paire UTF-16. Le dialogue système s'ouvre alors une seconde fois. En sélection multiple, l'application reçoit moins de fichiers, sans le savoir. Côté requête, `text_of` et `items::path` retirent eux aussi ces dossiers et ces noms.

### items.rs:49-70 — faible — plausible
Pour un fichier encore inexistant, l'élément est construit avec `SHSimpleIDListFromPath`, limité à MAX_PATH. Le préfixe `\\?\` et les chemins longs ne sont pas traités.
Scénario : une sauvegarde vers un chemin de plus de 260 caractères validée dans Oxyde fait échouer `GetResult`/`GetResults` avec E_FAIL. L'application signale une erreur alors que l'utilisateur a confirmé.

### items.rs:83-95 — faible — plausible
Le raccourci est résolu sur le thread de l'hôte sans pomper de messages. `is_file` est appelé sur la cible, et le tampon est limité à 1040 unités sans `Resolve`.
Scénario : un `.lnk` vers un partage hors ligne bloque l'application pendant le délai SMB. Une cible plus longue est tronquée ou un raccourci vers un dossier n'est pas suivi : l'application reçoit alors le `.lnk` lui-même au lieu de sa cible.

### ask.rs:210 — faible — confirmé
`AllowSetForegroundWindow(ASFW_ANY)` donne le droit de passer au premier plan à n'importe quel processus, pas seulement au lanceur ou à Oxyde.
Scénario : pendant l'ouverture du dialogue, un processus tiers peut voler le premier plan à l'application hôte.

### items.rs:62-70 — optimisation — confirmé
Pour un chemin inexistant, `item` analyse le chemin une première fois, puis `id_list` refait la même analyse avant le repli.
Scénario : à chaque `GetResult`, `GetCurrentSelection` ou `GetFolder` d'une sauvegarde, l'analyse est faite deux fois. Sur un partage réseau lent, le délai est doublé sur le thread de l'hôte.

<!-- LOT TERMINÉ -->
