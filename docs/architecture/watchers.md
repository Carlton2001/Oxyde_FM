# Surveillance des dossiers

**Statut : plan** (troisième domaine de l'option B, après les [volumes](volumes.md) et le
[presse-papiers](clipboard.md)). Ce document deviendra le contrat du domaine une fois
terminé.

## Périmètre

Le choix, pour chaque dossier surveillé (onglets, dossiers dépliés, racines des lecteurs,
tailles en direct), entre les événements du système et le sondage de `notify`. La file des
changements, le rafraîchissement des listes et les tailles de dossiers ne changent pas.

## Constat Linux (T11)

`reconcile_watchers` sonde un dossier si un lecteur qui le contient est USB ou de type de
support inconnu. Sous Linux, aucun lecteur n'a de type de support, et `/` contient tous les
chemins : **tous les dossiers sont sondés**, toutes les 2 s, et toutes les 30 s pour les
tailles en direct, qui parcourent alors tout le sous-arbre. Un fichier créé dans un
terminal apparaît donc jusqu'à 2 s plus tard, et les tailles ne profitent jamais des
événements.

Sous Windows, le sondage des clés USB évite de garder des handles qui bloquent l'éjection.
Sous Linux, inotify ne garde rien d'ouvert et n'empêche pas le démontage ; il voit aussi
les changements faits localement sur un montage FUSE (ntfs-3g). Il ne voit pas ceux d'une
autre machine sur un partage réseau.

## Interface

`Volumes` reçoit une méthode par défaut, qui porte la règle actuelle :

```rust
/// Whether `path` is watched by polling rather than system events.
fn polls(&self, path: &str, drives: &[DriveInfo]) -> bool;
```

- **Windows** : défaut, règle actuelle inchangée (lecteur USB ou de support inconnu).
- **Linux** : le montage le plus profond qui contient le chemin décide. Sondage pour les
  montages réseau (`drive_type == "remote"` : NFS, CIFS, sshfs…), événements partout
  ailleurs, y compris clés USB et NTFS.

Si inotify échoue (limite `max_user_watches` atteinte sur un très grand arbre), l'erreur
déjà gérée par `DirectoryWatcher` relance une lecture ; le repli automatique vers le
sondage n'est ajouté que si l'essai le montre nécessaire.

## Étapes

1. **Extraire la règle** dans `Volumes::polls`, sans changement : `drive_watchers.rs` seul.
2. **Règle Linux** et tests sur fixtures (`/`, `/run/media/…`, partage réseau imbriqué).
   Essai à la main : fichier créé dans un terminal (btrfs, Data, clé USB) visible tout de
   suite ; démontage et éjection d'une clé ouverte dans un onglet ; tailles en direct d'un
   dossier modifié.
3. **Bilan.**
