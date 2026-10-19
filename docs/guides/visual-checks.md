# Vérifier l’interface dans une session réelle

Choisir les scénarios concernés par le changement, sur des fixtures privées. Noter révision, Windows/cible, thème, langue, zoom, manipulation et résultat. Une compilation ou un test Win32 ne prouve pas le rendu GPUI.

## Parcours à choisir

| Changement | Essais utiles |
| --- | --- |
| Composants / formulaires | Thèmes clairs et sombres, cinq langues dont libellés longs, zoom, petites fenêtres, Tab/Entrée/Échap, erreurs et champs désactivés |
| Menus / fenêtres | Sous-menus après défilement, capture hors surface, champs texte/presse-papiers, clic propriétaire, Alt+Tab, fenêtre secondaire, plusieurs DPI ; rien ne réagit sous le menu |
| Aperçus | Images transparentes/animées, PDF, vidéo ; survol rapide, retour sur cache, source modifiée, défilement, fermeture, absence de résultat obsolète |
| Opérations | Progression, pause/reprise/annulation, conflits, reçus partiels, historique après relance, graphe final |
| Archives | Navigation, extraction, création/reconstruction des quatre formats modifiables, transferts, collisions, refus lisibles et récupération |
| Distribution | Installation propre et portable déplacé, caches/configuration, workers présents, ouverture de fichiers et fermeture |

Après [construction des binaires et workers](development.md), préparer le jeu d’archives sans lancer l’interface :

```powershell
cargo run --offline --example prepare_archive_isolation_visual
```

Le générateur crée un dossier neuf dans `target/` avec sources, références, archives normales et témoins de refus. Utiliser exclusivement ces copies. Pour les mutations, vérifier les octets/noms/dossiers vides ; après déplacement, seules les sources confirmées doivent disparaître. Une archive avec chemin interdit ne doit rien publier hors destination.

Ne pas provoquer de panne sur les documents personnels ni vider la Corbeille réelle pour un test. Si une petite opération finit trop vite pour essayer la pause, le signaler ; ne pas la présenter comme testée.

## Régression du ghost de glisser-déposer sous Windows

Ce test volontairement exclu de la suite ordinaire ouvre brièvement une fenêtre GPUI sans l’activer. Il démarre un drag synthétique, puis alterne Ctrl, Shift et leur relâchement sans mouvement du pointeur. Il vérifie à la fois le mode de dépôt et le mode utilisé par le rendu du ghost, sans effectuer de transfert :

```powershell
cargo test --offline --locked --bin oxyde native_drag_ghost_tracks_modifiers_without_pointer_movement -- --ignored --test-threads=1
```

Il couvre le routage GPUI et les notifications du ghost ; la réception des touches physiques par Windows et l’aspect des couleurs restent à vérifier manuellement. Les gestionnaires clavier appartiennent à la racine suivie par le focus : un `canvas` enfant possède son propre nœud de dispatch et ne reçoit pas les événements destinés à cette racine.

## Interpréter les résultats

- Les tests automatisés couvrent mieux délais, fautes et workers hostiles ; les essais manuels couvrent présentation, focus et interactions.
- Un retour utilisateur global ne prouve pas toutes les variantes de la matrice ; consigner sa portée réelle.
- La validation visuelle du 10 octobre 2026 concerne `243140d`. Elle ne valide pas rétroactivement les changements ultérieurs de confinement ou de fenêtres.
- Linux, RSS/GPU et comportements sur fournisseurs/volumes non essayés restent séparés. Voir les [limites de sécurité](../security/status.md) et les [bilans](../reports/README.md).
