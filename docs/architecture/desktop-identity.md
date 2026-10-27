# Identité de bureau Linux

**Statut : domaine terminé** (huitième domaine de l'option B, après les
[déplacements](moves.md)) ; bilan ci-dessous. Ce document est le contrat du domaine.

## Périmètre

Ce qui fait reconnaître Oxyde par GNOME et les autres bureaux freedesktop (point T15 de
l'audit) : identifiant d'application, entrée de bureau et icône. L'ouverture d'un dossier
passé en argument, la déclaration `inode/directory` (« Ouvrir avec ») et les métadonnées
AppStream attendent le paquet (T14). Windows garde son identité (`com.oxyde.desktop`,
icône `.ico` du binaire).

## Constat Linux

- Les fenêtres n'envoient aucun `app_id` Wayland : GNOME ne les rattache à aucune
  application, les montre sous une icône générique et ne les groupe pas dans le dock ou
  Alt+Tab.
- Aucune entrée `.desktop` ni icône hicolor : Oxyde est absent de la recherche des
  Activités et du lanceur.

## Règles

- Identifiant `io.github.Carlton2001.Oxyde`, sur le modèle des applications hébergées
  sur GitHub (convention Flathub) : `app_id` de toutes les fenêtres d'Oxyde, nom de
  l'entrée de bureau, de l'icône et `StartupWMClass` pour X11. Il ne change plus une fois
  publié.
- Entrée de bureau modelée sur celle de Nautilus (`Utility;Core;FileManager;`), nom et
  description traduits dans les cinq langues. `Exec` sans argument tant qu'Oxyde n'ouvre
  pas les chemins reçus.
- Icône : le logo SVG d'Oxyde (`assets/icons/logo_oxyde.svg`), celui dont dérive l'icône
  Windows, en taille libre dans `hicolor/scalable/apps`.
- Le script `scripts/install-linux-desktop.sh` installe ou retire l'entrée et l'icône
  pour l'utilisateur ; `--profile` fait lancer Oxyde sur un profil isolé, comme le profil
  de test de développement.

## Étapes

1. **Identité** : `APP_ID` sur chaque fenêtre, entrée `packaging/linux`, script
   d'installation utilisateur.
2. **Bilan.**

## Bilan

**Résultat.** Sous GNOME, Oxyde apparaît dans le dock, Alt+Tab et la recherche des
Activités avec son nom et son icône, et ses fenêtres secondaires (Propriétés, visionneuse,
opérations) sont regroupées avec la fenêtre principale ; essayé à la main sur Fedora 44.
L'envoi de l'`app_id` a été vérifié dans les échanges Wayland (`WAYLAND_DEBUG`), l'entrée
par `desktop-file-validate`.

**Fichiers communs touchés** : `src/ui/mod.rs` (`APP_ID`) et chacune des 14 créations de
fenêtre, qui portent toutes l'identifiant ; Windows l'ignore.

**Leçons.**
- GNOME ne laisse pas interroger ses fenêtres (`org.gnome.Shell.Introspect` refusé hors
  applications autorisées) : `WAYLAND_DEBUG=client` est le moyen de vérifier ce qu'une
  fenêtre annonce.
- Le lanceur de développement doit garder le profil isolé : l'entrée installée par le
  script le fixe par variables d'environnement dans `Exec`.

**Reporté au paquet (T14).** Ouvrir un dossier reçu en argument, puis déclarer
`inode/directory` pour « Ouvrir avec » ; métadonnées AppStream ; icônes PNG pour les
bureaux sans SVG.
