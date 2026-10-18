# Référence Tauri du benchmark

Source locale :
`C:/Users/Biluf/Documents/Projects/Biluf_FM_Clean/src-tauri/src/commands/icons.rs`.
Révision Git : `54b8b4c2586908c29d0e60af0c4ec8d7c5c49dd4`, fichier sans modification
locale lors de sa copie. SHA-256 du fichier complet original :
`3d3800a27412f351da198ba9f611f235272214580b87b737074add5d10861a37`.

`source.rs` garde l'extraction, la conversion GDI, le recadrage, les filtres et
le cache Tauri. Les adaptations sont limitées aux imports `CommandError` et
`AppHandle`, aux attributs `#[tauri::command]` et au retrait du point d'entrée
`get_file_icon(AppHandle, ...)`. Le benchmark appelle directement
`extract_icon_png` avec un répertoire temporaire. Le projet Tauri n'est pas
modifié. Garder cette référence figée, sans formatage automatique de son corps.

Les deux méthodes sont compilées dans le même binaire avec les dépendances et
le profil de GPUI. Il s'agit d'une comparaison des méthodes, pas des deux
applications complètes ni de leurs versions de bibliothèques respectives.

```powershell
cargo test --offline --locked --lib platform::icons::tauri_comparison_tests::compare_tauri_and_gpui_icons -- --ignored --exact --test-threads=1 --nocapture
```

Utiliser un TEMP/TMP privé et les permissions natives de création des fichiers
temporaires. `OXYDE_ICON_BENCH_REPORT` choisit le fichier JSON des mesures brutes.

Les cinq phases comparées sont :

1. Extraction seule, sans disque, sortie PNG pour les deux méthodes.
2. Cache Oxyde vide, avec la persistance propre à chaque méthode.
3. Même extraction à cache vide, Tauri utilisant cette fois la publication de
   cache contrôlée et synchronisée de GPUI.
4. Cache disque chaud : chemin Tauri retournant le PNG, chemin du parent GPUI
   retournant les pixels décodés.
5. Cache mémoire du backend/worker rempli, sans couvrir les caches de l'interface.

Le décodage du résultat PNG Tauri est hors chronométrage dans les phases 2–5,
comme le décodage WebView n'appartient pas au backend. Une comparaison de pixels
hors chronométrage relève toute différence entre les sorties. Les quatre
échauffements sont exclus ; l'ordre Tauri/GPUI alterne et les opérations de
vidage des caches restent hors chronométrage. Le Shell et le cache système sont
chauds. Le démarrage des processus, IPC, files et rendu ne sont pas mesurés.
