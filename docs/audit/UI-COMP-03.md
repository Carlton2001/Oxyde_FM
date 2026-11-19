# Audit UI-COMP-03

## Fichiers lus en entier

- [x] `src/ui/components/input.rs`
- [x] `src/ui/components/input/completion_menu.rs`
- [x] `src/ui/components/input/completion_menu/scroll.rs`
- [x] `src/ui/components/input/field.rs`
- [x] `src/ui/components/input/options.rs`
- [x] `src/ui/components/input/render.rs`
- [x] `src/ui/components/input/selection.rs`
- [x] `src/ui/components/mod.rs`
- [x] `src/ui/components/modal.rs`
- [x] `src/ui/components/network.rs`
- [x] `src/ui/components/network_dialog.rs`
- [x] `src/ui/components/network_dialog/letters.rs`
- [x] `src/ui/components/open_with_dialog.rs`
- [x] `src/ui/components/operations.rs`
- [x] `src/ui/components/operations/compact.rs`
- [x] `src/ui/components/operations/detail.rs`
- [x] `src/ui/components/operations/detail/controls.rs`
- [x] `src/ui/components/operations/detail/graph.rs`
- [x] `src/ui/components/operations/detail/metrics.rs`
- [x] `src/ui/components/operations/footer.rs`
- [x] `src/ui/components/operations/items.rs`

## Constats

### Élevé

1. `src/ui/components/input.rs:771-808`, élevé, plausible. Le champ ne suit jamais de plage de composition : la plage marquée est toujours absente, la fin de composition ne fait rien, et le texte en cours de composition est inséré comme du texte définitif. Scénario : sous Windows (et de même avec un IME Linux), un utilisateur saisit du japonais, du chinois ou du coréen dans un champ (renommage, barre d'adresse, filtre). Chaque mise à jour de la préédition est ajoutée à la suivante, puis le résultat validé est inséré encore une fois. Le texte obtenu est dupliqué et illisible. Le système n'a aucune plage à remplacer, car le champ répond « aucune » quand on la lui demande.

### Moyen

2. `src/ui/components/input/render.rs:80-88`, moyen, confirmé. Le bouton d'effacement vide le texte et ramène la sélection à zéro, mais laisse l'ancre de sélection, la dernière mise en page et la liste de complétions à leur valeur précédente. Le clic est arrêté sur le bouton, donc le gestionnaire du champ ne remet pas l'ancre à jour. Scénario : saisir « abcde », cliquer sur la croix, puis Maj+clic dans le champ. La sélection devient 0..5 sur un texte vide, et la frappe suivante remplace une plage hors limites, ce qui fait paniquer le processus. Le cas se présente dans tout champ à bouton d'effacement, par exemple le filtre de la fenêtre des opérations.

3. `src/ui/components/input.rs:459-472` (et `749-759`), moyen, confirmé. Un champ masqué (mot de passe de lecteur réseau) autorise Ctrl+C, Ctrl+X, le menu contextuel et la sélection de mot, et copie le texte en clair dans le presse-papiers. Il fournit aussi son contenu en clair aux requêtes de texte du système. Scénario : l'utilisateur sélectionne tout dans le champ de mot de passe et fait Ctrl+C. Le mot de passe se retrouve dans le presse-papiers et dans son historique. Un champ natif de mot de passe refuse cette copie.

### Faible

4. `src/ui/components/input.rs:358-368` et `779-798`, faible, plausible. L'index d'un clic vient de la dernière mise en page, qui n'est pas invalidée après une frappe (seuls `set_content` et `enter` la remettent à zéro). L'index est seulement borné à la longueur, pas à une frontière de caractère. Les plages fournies par le système ne sont pas non plus vérifiées (début supérieur à la fin, par exemple). Scénario : une frappe puis un clic arrivent avant le prochain rendu sur un texte accentué. La sélection tombe au milieu d'un caractère, et la modification suivante, ou le calcul de l'index masqué, panique.

5. `src/ui/components/input.rs:1030` et `1040`, faible, plausible. Les résultats du dessin de l'arrière-plan et du texte sont déballés sans gestion d'erreur. Scénario : un échec d'allocation de texture ou de glyphe (mémoire GPU saturée, périphérique perdu) interrompt tout le processus en plein rendu au lieu d'omettre le texte.

6. `src/ui/components/input.rs:519-568`, faible, confirmé. Retour arrière au début du champ ou Suppr à la fin, sans sélection, ne modifient rien mais déclenchent quand même le rappel de changement et le rafraîchissement des complétions. Scénario : dans le filtre de la fenêtre des opérations, Retour arrière sur un champ vide relance le tri en arrière-plan et ramène la liste en haut (`items.rs:269-272`). La barre d'adresse relance, elle, un listage de dossier.

7. `src/ui/components/input.rs:129-132`, faible, confirmé. La saisie de navigation est rognée de ses espaces avant et après. Scénario : sous Linux, un dossier dont le nom finit par une espace est légal. Le saisir dans la barre d'adresse mène vers un autre chemin, ou vers un chemin invalide.

8. `src/ui/components/network_dialog.rs:231-243`, faible, plausible. Le clic sur une ligne de lecteur indexe la liste des focus par la position capturée au rendu. Cette liste est recréée, avec une taille possiblement réduite, après le chargement ou après une déconnexion (`app/network.rs:294` et `509`). Scénario : un clic reçu entre la mise à jour des lecteurs et le rendu suivant, sur un index qui n'existe plus, panique par accès hors bornes.

9. `src/ui/components/operations/items.rs:233-255`, faible, confirmé. La fin du chargement applique l'ordre calculé avec le filtre et le tri capturés au lancement, sans comparer à l'état actuel. Scénario : pendant le chargement (nombreuses sources sur un lecteur réseau lent), l'utilisateur tape un filtre ou clique sur un en-tête. Le résultat tardif écrase le résultat récent, et la liste affichée ne correspond plus au texte du filtre ni à la colonne de tri indiquée.

10. `src/ui/components/operations/items.rs:244` et `267`, faible, plausible. Le tri par état utilise les résultats par élément pris au moment du calcul, et rien ne recalcule la liste quand ils évoluent. Scénario : une opération en cours est triée par état. Les éléments passent de « en attente » à « terminé », mais les lignes gardent leur ordre initial jusqu'à ce que l'utilisateur change le filtre ou le tri.

### Optimisation

11. `src/ui/components/input.rs:134-140` et `626-647`, optimisation, confirmé. Chaque complétion liste le dossier en entier, avec tri et métadonnées (la variante triée), puis refiltre et retrie. Sous Windows, le préfixe est passé en minuscules pour chaque entrée. Scénario : un dossier de 100 000 entrées. Chaque pause de frappe de plus de 150 ms coûte un listage, deux tris et 100 000 allocations. Le listage non trié existe déjà, et les tâches de listage lancées en bloquant ne sont pas interrompues quand une nouvelle frappe arrive.

12. `src/ui/components/open_with_dialog.rs:28`, `108`, `113` et `159`, optimisation, confirmé. La liste visible, qui passe en minuscules et retire les accents du nom de chaque application, est recalculée au moins trois fois par rendu. Elle l'est aussi à chaque calcul de ligne (`row_child`). Scénario : sur un système avec plusieurs centaines d'applications, chaque frappe dans la recherche ou chaque déplacement de sélection refait des milliers d'allocations au lieu d'un seul calcul.

13. `src/ui/components/operations/items.rs:99`, optimisation, confirmé. Le tri par lieu passe les deux dossiers en minuscules dans le comparateur, donc deux allocations par comparaison. Scénario : tri par la colonne « lieu » sur une opération de 100 000 éléments, environ deux millions d'allocations. Le champ minuscule pourrait être calculé une fois avec l'élément.

14. `src/ui/components/operations/items.rs:225-230` et `263`, `277-284`, optimisation, confirmé. Le fil d'interface clone la liste des sources et la table des résultats de l'opération, sous les verrous globaux du gestionnaire que les threads de copie utilisent aussi. La table des résultats est reclonée à chaque frappe du filtre. Scénario : opération de plusieurs centaines de milliers de fichiers. Le clonage bloque l'interface et entre en conflit avec les mises à jour de progression.

15. `src/ui/components/operations/items.rs:237-243`, optimisation, plausible. Les métadonnées des sources sont lues l'une après l'autre dans une seule tâche, sans plafond ni lecture parallèle, et la liste n'apparaît qu'à la fin. Scénario : plusieurs milliers de sources sur un partage réseau lent. La liste reste sur « chargement » pendant des minutes.

16. `src/ui/components/operations/footer.rs:36-40` et `65-71`, optimisation, plausible. L'animation de respiration, répétée tant qu'une opération est active, demande une image à chaque frame, ce qui redessine la vue de l'application entière (la barre de titre est rendue dans la vue principale). À chaque rendu, le compteur d'opérations verrouille toutes les opérations. Scénario : pendant une longue copie, l'application est reconstruite à la cadence de l'écran au lieu de ne l'être qu'aux changements de progression. Le coût en processeur augmente et les verrous des threads de copie sont disputés.

17. `src/ui/components/modal.rs:148-150`, optimisation, plausible. Le titre de fenêtre est défini dans le rendu, donc à chaque rendu d'une boîte sans en-tête personnalisé. Sous Windows, cela appelle le changement de titre du système (et met à jour l'accessibilité) sans comparaison avec la valeur précédente. Scénario : dans les boîtes de dialogue, chaque frappe ou chaque image d'animation répète l'appel.

<!-- LOT TERMINÉ -->
