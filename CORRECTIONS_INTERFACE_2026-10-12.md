# Corrections d’interface — 12 octobre 2026

Les changements sont implémentés dans le workspace. La compilation et les tests ci-dessous ont été exécutés sous Windows. Le rendu et les manipulations réelles restent à vérifier dans l’application ; aucune validation visuelle automatisée n’a été possible dans cette session.

| Demande | Changement |
| --- | --- |
| Colonne Type / Taille rétrécie | Les libellés d’en-tête peuvent se comprimer et affichent une ellipse ; le retour à la ligne est explicitement interdit. |
| Dépôt sur soi | Les dépôts internes retirent les sources identiques à la cible ou déjà présentes dans le dossier cible, avant la préparation d’un transfert. Cela couvre la liste, la grille, la sidebar et les onglets existants. Copier/Coller conserve la copie avec un nouveau nom. |
| Focus après redimensionnement vertical | Le curseur de sélection est remis en vue en utilisant les lignes réellement affichées, y compris les groupes de dates et la grille. |
| Glissement hors fenêtre | La capture native Windows conserve les événements jusqu’au relâchement. Les mouvements sont suivis au niveau de la fenêtre, sans annulation lors de la sortie. Le relâchement extérieur termine également l’interaction. |
| Ignorer tout | Les sources ignorées ne sont pas mises en file. Quand aucune source ne reste, une information « Tous les éléments ont été ignorés » remplace la confirmation de copie terminée. Les sources sans conflit sont conservées. |
| Breadcrumb Corbeille | Le point d’entrée d’édition refuse `trash://`. |
| Éjection USB | Le choix du nœud PnP se base sur `CM_DRP_CAPABILITIES` / `CM_DEVCAP_REMOVABLE`, plutôt que sur `DN_REMOVABLE`. Les watchers restent libérés avant l’éjection et les codes de refus sont journalisés. |
| Ascenseur horizontal sidebar | Correction du signe de l’offset fourni au témoin. |
| Dépliant des épinglés | Le menu est ancré aux bounds du bouton. Un deuxième clic le ferme ; Échap, le clic extérieur et le changement de mode sidebar le ferment aussi. |
| Clic milieu sidebar réduite | Les lecteurs, le réseau, la corbeille et les lignes du dépliant des épinglés ouvrent un nouvel onglet. |
| Menus contextuels sidebar réduite | Réutilisation des actions existantes, en retirant Tout développer / Tout réduire. Les lignes du dépliant ont également leur menu contextuel. |
| Dossier déposé sur l’espace libre des onglets | Les dossiers de la sélection ouvrent de nouveaux onglets dans le panneau cible. Le ghost affiche « Créer un nouvel onglet ». Sur un onglet existant, il retrouve Copier / Déplacer. |
| Défilement des onglets | Animation de 180 ms avec ralentissement progressif pour les flèches et la molette ; les gestes successifs ajustent la destination. |
| Espace vide en fin de liste | La même hauteur vide défilante est utilisée en Détails et Grille (26 pixels à l’échelle 16), avec ou sans regroupement. La grille virtualise ses lignes tout en conservant ce petit espace final de hauteur différente. |
| Dernier élément sous la barre d’état | Le contenu du panneau occupe l’espace restant après les onglets ; la liste Détails réserve explicitement la barre d’état et l’ascenseur horizontal. La géométrie de sélection et le déplacement au clavier suivent la zone visible. |
| Menu Exécuter des raccourcis | Le clic extérieur sur le champ ne ferme plus le menu avant que le champ traite son second clic. La sélection reste inchangée. |
| Plusieurs propriétés | Fenêtres natives indépendantes utilisant `tool_window`, comme Recherche avancée. Sous Windows, elles possèdent Oxyde comme propriétaire natif et restent au-dessus de lui après un clic dans sa fenêtre, sans priorité globale sur les autres applications. Propriétés Système ferme uniquement la fenêtre concernée ; la fermeture principale les ferme toutes. |
| Calculs de propriétés indépendants | Chaque fenêtre possède ses métadonnées, ses champs et sa tâche annulable. Chaque parcours réserve aussi son propre pool de workers, car jwalk retenait tous les workers du pool commun jusqu’à la fin du premier parcours. Deux calculs physiques restent possibles simultanément, avec 16 workers SSD ou 2 HDD au total. La cellule affiche « Calcul de la taille… ». La fermeture abandonne le calcul de cette fenêtre. |

## Validation exécutée

- `cargo check --offline` : succès, sans avertissement du projet.
- `cargo build --offline` : succès ; exécutable disponible dans `target/debug/oxyde.exe`.
- `cargo test --offline --bin oxyde` : 175 tests réussis, un test de profilage déjà ignoré, dont trois nouveaux tests de géométrie de grille.
- Capture native Windows : un test réussi, avec un relâchement aux coordonnées extérieures à la fenêtre.
- Transferts dans le même dossier : trois tests réussis ; la copie explicite continue à créer les noms numérotés et le déplacement sur soi conserve les sources.
- Contrôles d’éjection Windows : deux tests réussis, couvrant les cibles invalides et la libération des notifications.
- Les catalogues des cinq langues sont couverts par les tests d’interface.
- Calculs de tailles : cinq tests réussis, dont un test avec tous les workers du premier parcours volontairement bloqués et un second petit dossier qui doit finir ; un profilage reste ignoré. Trois tests supplémentaires couvrent l’API de calcul et ses totaux indépendants.
- Propriétaire natif Windows : un test réussi, avec deux fenêtres possédées par la même fenêtre principale qui reste activable, sans drapeau global « toujours au-dessus ».
- `git diff --check` : aucune erreur d’espacement.

## Vérifications interactives restantes

Vérifier les colonnes étroites, les ascenseurs et redimensionnements hors fenêtre, les menus de la sidebar réduite, les dépôts sur la barre d’onglets, la dernière ligne en Détails et Grille, puis plusieurs fenêtres de propriétés dont un gros dossier et un raccourci. Refaire les contrôles visuels en thème clair/sombre et avec une autre échelle d’interface.

Aucun lecteur USB n’était présent lors de l’inventaire matériel : seulement deux SSD NVMe internes. L’éjection réelle et la comparaison avec l’Explorateur restent donc à confirmer avec une clé branchée. La cible Linux n’a pas été compilée dans cette session.

Références Windows utilisées : [capture souris](https://learn.microsoft.com/en-us/windows/win32/learnwin32/mouse-movement), [fenêtres possédées](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features), [capacités d’un périphérique](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-capabilities), [éjection PnP](https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_request_device_ejectw).
