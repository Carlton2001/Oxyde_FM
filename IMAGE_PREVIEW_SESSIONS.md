# Sessions d’aperçus images Windows

Depuis le 14 octobre 2026, les demandes `image` et `image-thumbnail` utilisent
une session séquentielle de `oxyde-image-worker.exe`, démarrée à la demande.
Le premier fichier paie la création du bac à sable et la copie du runtime privé ;
les demandes suivantes réutilisent le même processus. Les caches mémoire et les
miniatures disque conservent leur fonctionnement et leurs limites.

## Cycle de vie et protocole

- Un seul worker image persistant ; les demandes restent sérialisées par
  l’admission existante. Les opérations restent hors du thread d’interface.
- Arrêt après 15 secondes d’inactivité, vérifié chaque seconde. La session est
  également remplacée après 32 demandes ou dès que son âge atteint 60 secondes
  entre deux demandes. Une demande active garde son délai propre de 15 secondes.
- La limite mémoire de 512 Mio et la réservation du budget global restent actives,
  y compris pendant l’attente. Une pression de ressources ferme la session idle.
- Le parent ouvre et retient la source via `Platform`, puis duplique uniquement son
  handle en lecture seule dans le worker, sans héritage ni handle du parent.
  Aucun chemin n’est transmis. Le worker normal ferme sa source avant le décodage.
- Chaque commande fait 17 octets : identifiant `u64`, handle `u64`, mode `u8`.
  Le worker accepte au maximum 32 commandes ordonnées. Leur volume total reste
  inférieur à la capacité du pipe, même si un worker compromis refuse de le lire.
- Les réponses contiennent l’identifiant, une longueur `u32` bornée par `MAX_REPLY`,
  puis le protocole existant de pixels bruts. Le parent valide les identifiants,
  tailles, dimensions et budgets ; les réponses texte sont refusées.
- Tout délai dépassé, crash, erreur de transport/protocole ou erreur de décodage
  retire la session. La demande suivante recrée un processus ; aucun décodage
  local permissif ni nouvelle tentative automatique du fichier fautif.
- PDF, vidéo, Office et archives gardent leurs processus et parcours distincts.
  Le moteur SVG et ses polices sont initialisés seulement au premier SVG.

## Garanties conservées et compromis

Le lancement utilise le même LPAC sans capacités, son runtime privé, ses seuls
handles IPC explicites, son Job Object attaché avant reprise et ses plafonds.
La réutilisation n’accorde aucun accès supplémentaire aux chemins utilisateur,
aucune écriture sur la source, aucun réseau ni accès au processus parent.

Elle abandonne toutefois la remise à zéro de la mémoire entre deux images : un
décodeur compromis peut conserver des contenus/handles et falsifier les résultats
des images suivantes **dans cette session**. Les limites de demandes, d’âge et
d’inactivité réduisent cette fenêtre, sans restaurer l’isolation par document.
Le recyclage supprime cet état ; le cache parent ne stocke que les résultats
validés, avec ses limites existantes. Les tests ne prouvent pas l’absence de
vulnérabilité des codecs ni une étanchéité absolue du bac à sable Windows.

La fermeture finale demande l’arrêt du superviseur et interrompt ses lectures
surveillées. Elle attend au maximum deux secondes avant le contournement natif
existant de sortie du processus. Le Job Object ferme le worker si le parent
disparaît ; un arrêt brutal peut laisser des fichiers de runtime temporaires,
comme les autres workers. Voir `SHUTDOWN_DIAGNOSTICS.md`.

## Vérification et reconstruction

Fermer Oxyde avant de reconstruire tous ses exécutables :

```powershell
cargo build --offline --bins
cargo test --offline --lib platform::windows_preview_worker -- --test-threads=1 --nocapture
cargo run --offline
```

Les tests exercent de vrais processus Windows : réutilisation du PID, PNG/SVG et
miniatures successifs, absence de croissance des handles, libération des sources,
droits de lecture seule, recyclage après erreur/32 demandes, délais et arrêt du
service. Les mesures froid/chaud du petit PNG sont indicatives, sans seuil de
performance fragile dans les assertions. La fluidité réelle au survol, les grands
fichiers et la fermeture de l’application restent à confirmer visuellement.

Validation du 14 octobre 2026 : `cargo check --offline --bins` sans avertissement,
12 tests `platform::windows_preview_worker` et 7 tests
`platform::windows_worker_sandbox::security_tests` réussis. Lors du dernier
passage, le petit PNG du test prend environ 1,51 seconde avec création de session,
puis 2,43 millisecondes pour le fichier suivant dans le même processus. Cette
mesure inclut le transport et ne prédit pas le temps de décodage des grands fichiers.
