common-cancel = Cancelar

rename-title = Cambiar nombre

rename-name-placeholder = Nombre del archivo

rename-error-empty-name = El nombre no puede estar vacío.

rename-error-already-exists = Ya existe aquí un archivo o una carpeta con este nombre.

rename-error-not-found = El elemento que se va a renombrar ya no existe o no está accesible.

rename-error-access-denied = No se permite cambiar el nombre. Comprueba los permisos de este elemento.

rename-error-invalid-name = No se pudo cambiar el nombre. Compruébalo e inténtalo de nuevo.

rename-error-unavailable = Este elemento no se puede renombrar ahora.

rename-error-task-failed = No se pudo cambiar el nombre. Inténtalo de nuevo.

conflict-title = Los archivos ya existen

conflict-description = { $count ->
    [one] Se ha detectado { $number } conflicto. ¿Qué quieres hacer?
   *[other] Se han detectado { $number } conflictos. ¿Qué quieres hacer?
    }

conflict-skip-all = Omitir todos

conflict-replace-all = Reemplazar todo

common-close = Cerrar

common-retry = Reintentar

settings-title = Preferencias

settings-appearance = Apariencia

settings-theme = Tema

settings-scale = Escala de la interfaz

settings-scale-value = { $reference } px

settings-scale-reset = Restablecer la referencia a 16 px por rem

settings-language = Idioma

theme-dark = { $name } Oscuro

theme-light = { $name } Claro

settings-files = Archivos

settings-hidden = Archivos ocultos

settings-hidden-description = Mostrar archivos y carpetas ocultos, incluidos los nombres que empiezan por un punto

settings-system = Archivos de sistema

settings-system-description = Mostrar archivos y carpetas marcados como elementos del sistema por el SO

settings-previews = Vistas previas de medios

settings-previews-description = Mostrar vistas previas de archivos en la información emergente de la lista

settings-grid-thumbnails = Miniaturas de imágenes en la cuadrícula

settings-grid-thumbnails-description = Mostrar miniaturas de imágenes directamente en las tarjetas de la cuadrícula

settings-about = Acerca de

settings-version = Versión { $version }


settings-save-error = Las preferencias se han aplicado, pero no se han podido guardar. Inténtalo de nuevo.

settings-load-error = No se pudieron leer las preferencias guardadas. Los cambios no se guardarán para proteger ese archivo.

status-folders = { $count ->
    [one] { $number } carpeta
   *[other] { $number } carpetas
    }

status-files = { $count ->
    [one] { $number } archivo
   *[other] { $number } archivos
    }

view-details = Vista de detalles

view-grid = Vista de cuadrícula

selected-files = { $count ->
    [one] { $number } archivo seleccionado ({ $size })
   *[other] { $number } archivos seleccionados ({ $size })
    }

selected-folders = { $count ->
    [one] { $number } carpeta seleccionada ({ $size })
   *[other] { $number } carpetas seleccionadas ({ $size })
    }

selected-mixed = { $folder_count ->
    [one] { $folders } carpeta
   *[other] { $folders } carpetas
    } y { $file_count ->
    [one] { $files } archivo
   *[other] { $files } archivos
    } seleccionados ({ $size })

tooltip-type = Tipo: { $value }

tooltip-size = Tamaño: { $value }

tooltip-modified = Modificado: { $value }

tooltip-location = Ubicación: { $value }

menu-open = Abrir

menu-open-new-tab = Abrir en nueva pestaña

menu-pin-quick-access = Anclar al acceso rápido

menu-unpin-quick-access = Desanclar del acceso rápido

menu-cut = Cortar

menu-copy = Copiar

menu-paste = Pegar

menu-rename = Cambiar nombre

menu-delete = Eliminar

menu-properties = Propiedades

menu-more-options = Más opciones

menu-new-folder = Nueva carpeta

menu-duplicate-tab = Duplicar pestaña

menu-close-other-tabs = Cerrar otras pestañas

menu-close-tab = Cerrar pestaña

menu-select-all = Seleccionar todo

menu-select-all-checks = Marcar todos

menu-deselect-all-checks = Desmarcar todos

column-name = Nombre

column-type = Tipo

column-size = Tamaño

column-modified = Modificado

file-type-folder = Carpeta

file-type-file = Archivo

file-type-symlink = Enlace simbólico

file-type-junction = Unión de directorios

empty-filtered = Ningún elemento coincide con los filtros.

folder-loading = Cargando carpeta…

folder-empty = Esta carpeta está vacía.

preview-loading = Cargando vista previa…

operation-failed = La operación ha fallado

path-placeholder = Ruta

navigation-back = Atrás

navigation-forward = Adelante

navigation-refresh = Actualizar

navigation-undo = Deshacer

navigation-redo = Rehacer

operation-copy = Copiar

operation-move = Mover

operation-rename = Cambiar nombre

operation-delete = Eliminar

operation-compress = Compresión

operation-extract = Extracción

operation-extract-here = Extraer archivo comprimido

operation-new-folder = Nueva carpeta

operation-restore = Restaurar

operations-title = { $count ->
    [1] Operación
   *[other] Operaciones
    }

operations-show-panel = Mostrar operaciones

operations-hide-panel = Ocultar operaciones

operations-queued = En cola

operations-calculating = Calculando...

operations-running = En curso

operations-paused = Pausado

operations-cancelled = Cancelada

operations-completed = Completado

operations-error = Fallida

operations-from = Desde: { $path }

operations-to = Hasta: { $path }

operations-items = { $done } / { $total } elementos

operations-remaining = Tiempo restante: { $seconds } s

operations-duration = Duración: { $duration }

operations-average = Promedio: { $speed }

operations-speed-waiting = Esperando...

operations-pause = Pausar

operations-resume = Reanudar

operations-stop = Cancelar operación

operations-fast = Modo rápido activo — haz clic para cambiar al modo normal

operations-normal = Modo normal activo — haz clic para cambiar al modo rápido

operations-dismiss = Quitar

operations-dismiss-all = Quitar todas las operaciones finalizadas

operations-empty = No hay operaciones

name-filter-placeholder = Filtrar por nombre

undo-operation = Deshacer: { $operation }

redo-operation = Rehacer: { $operation }

shortcut-back = Alt+Izquierda

shortcut-forward = Alt+Derecha

sidebar-quick-access = Acceso rápido

sidebar-drives = Unidades

sidebar-locations = UBICACIONES

sidebar-network = Red

sidebar-trash = Papelera

size-tiny = Minúsculo (< 10 KB)

size-small = Pequeño (10 KB - 1 MB)

size-medium = Mediano (1 MB - 100 MB)

size-large = Grande (100 MB - 1 GB)

size-huge = Gigante (> 1 GB)

date-today = Hoy

date-yesterday = Ayer

date-this-week = Esta semana

date-last-week = La semana pasada

date-older = Más antiguo

path-error-not-found = No se encontró esta carpeta o ya no está disponible.

path-error-access-denied = Acceso denegado a esta carpeta. Comprueba tus permisos.

path-error-failed = No se pudo abrir esta carpeta. Comprueba la ruta e inténtalo de nuevo.

undo-failed = No se pudo deshacer: { $detail }

undo-overwrite-unavailable = Esta copia no se puede deshacer porque ha reemplazado archivos existentes.

redo-failed = No se pudo rehacer: { $detail }

prepare-failed = No se pudo preparar la operación: { $detail }

transfer-failed = La transferencia ha fallado: { $detail }

drag-copy = Copiar

drag-move = Mover

new-folder-name = Nueva carpeta

new-folder-numbered = Nueva carpeta ({ $number })

native-menu-empty = No hay comandos disponibles

native-menu-failed = No se pudo cargar el menú del sistema: { $detail }

drag-items = { $count ->
    [one] { $number } elemento
   *[other] { $number } elementos
    }

native-menu-loading = Cargando menú…

local-drive = Disco local ({ $drive })

unknown-item = Desconocido

menu-expand-all = Expandir todo

menu-collapse-all = Contraer todo

menu-copy-path = Copiar ruta

sidebar-collapse-all = Contraer todo

sidebar-scroll-top = Volver arriba

sidebar-collapse = Contraer panel lateral

sidebar-expand = Expandir

settings-windows-icons = Iconos de Windows

settings-windows-icons-description = Usar los iconos de archivos y carpetas de Windows en las vistas de lista y cuadrícula.

settings-checkboxes = Casillas de selección

settings-checkboxes-description = Seleccionar varios elementos sin mantener pulsado Ctrl.

settings-path-tracking = Seguir ruta activa

settings-path-tracking-description = Expandir las carpetas superiores y mostrar la carpeta activa en el panel lateral.

settings-date-format = Formato de fecha

date-format-us = Estados Unidos (MM/DD/AAAA)

date-format-european = Europeo (DD/MM/AAAA)

date-format-iso = ISO 8601 (AAAA-MM-DD)

settings-network = Red

settings-network-description = Mostrar el entorno de red en el panel izquierdo.

settings-region-language = Región e Idioma

settings-performance = Rendimiento

settings-turbo = Transferencias Turbo

settings-search-limit = Límite de búsqueda

settings-data = Datos de la aplicación

settings-clear-cache = Borrar caché

settings-reset-preferences = Restablecer preferencias

title-new-panel = Nuevo panel a la derecha

title-tools = Herramientas

tool-sizes = Calcular tamaños

tool-search = búsqueda avanzada

quick-search = Búsqueda rápida

quick-search-title = Buscar «{ $query }» en { $folder }

quick-search-results = Resultados de búsqueda

quick-search-running = Buscando…

quick-search-limited = Límite de resultados alcanzado

quick-search-empty = Sin resultados

quick-search-failed = No se pudo buscar

quick-search-clear = Borrar

quick-search-stop = Detener búsqueda

open-file-location = Abrir ubicación del archivo

column-location = Ubicación

tool-duplicates = Encontrar duplicados

tabs-new-tab = Nueva pestaña

date-this-month = Este mes

settings-group-by-date = Agrupar por fecha

settings-group-by-date-description = Agrupar los archivos por fecha de modificación en la pestaña activa.

date-group-count = { $count ->
    [one] { $number } elemento
   *[other] { $number } elementos
    }

menu-sort-by = Ordenar por

menu-sort-ascending = Ascendente

menu-sort-descending = Descendente

menu-sort-extension = Extensión

drive-eject = Expulsar

map-network-drive = Conectar a unidad de red

disconnect-network-drive = Desconectar unidad de red

disconnect-this-network-drive = Desconectar esta unidad de red

network-force-prompt = Hay archivos abiertos u operaciones en curso en { $drive }. Forzar la desconexión puede interrumpir una copia o perder cambios sin guardar. ¿Quieres continuar?

network-force-action = Forzar desconexión

network-category = Categoría

tooltip-network-category = Categoría: { $value }

tooltip-network-workgroup = Grupo de trabajo: { $value }

tooltip-network-location = Ubicación de red: { $value }

network-workgroup = Grupo de trabajo

network-location = Ubicación de red

network-computer = Equipo

network-other-devices = Otros dispositivos

network-drive-failed = No se pudo cambiar la conexión de red. { $detail }

open-item-failed = No se pudo abrir este elemento. { $detail }

network-map-description = ¿Qué carpeta de red le gustaría conectar?

network-map-hint = Indica la letra de unidad y la carpeta que quieres conectar.

network-disconnect-description = Selecciona las unidades de red que quieres desconectar y haz clic en Desconectar.

network-letter = Unidad

network-folder = Carpeta

network-username = Usuario

network-password = Contraseña

network-reconnect = Volver a conectar al iniciar sesión

network-finish = Finalizar

network-disconnect = Desconectar

network-no-drives = No se encontraron unidades de red.

network-no-letter = No hay ninguna letra de unidad disponible.

network-invalid-path = Introduce una ruta de red válida, por ejemplo \\servidor\recurso.

network-connected = ¡Unidad de red conectada con éxito!

network-disconnected = Unidades de red desconectadas correctamente.

network-working = Procesando…

network-loading = Cargando unidades…

drive-eject-failed = No se pudo expulsar la unidad. Puede que otra aplicación la esté usando. { $detail }

archive-format = Archivo comprimido .{ $format }

compression-fast = Rápida

compression-normal = Equilibrada

compression-best = Máxima

compress = Comprimir

compression = Compresión

compression-format = Formato

disk-image-mount = Montar

disk-image-unmount = Desmontar

disk-image-mount-failed = No se pudo montar la imagen. { $detail }

disk-image-unmount-failed = No se pudo desmontar la imagen. { $detail }

disk-image-open-failed = No se pudo abrir la imagen. { $detail }

disk-image-busy = Una operación de archivos sigue usando esta imagen.

disk-image-no-volume = No se ha asignado ningún volumen accesible a esta imagen.

advanced-settings = Configuración

advanced-help = Ayuda

advanced-search-for = Buscar por

advanced-search-for-placeholder = *.* o nombre.txt

advanced-search-in = Buscar en

advanced-search-subfolders = Buscar en subcarpetas

advanced-search-in-archives = Buscar dentro de archivos

advanced-find-text = Buscar texto en el archivo

advanced-find-text-placeholder = Buscar dentro de archivos (no binarios)...

advanced-global-options = Opciones globales

advanced-case-sensitive = Distinguir mayúsculas

advanced-ignore-accents = Ignorar acentos

advanced-reset = Restablecer

advanced-search = Buscar

advanced-regex-help-title = Patrones comunes

advanced-use-regex = Expresión regular

advanced-regex-any-char = Cualquier carácter

advanced-regex-any-char-desc = Coincide con cualquier carácter. Ejemplo: "b.t" coincide con "bat", "bit", "but".

advanced-regex-digit = Dígitos

advanced-regex-digit-desc = Coincide con cualquier número del 0 al 9. Ejemplo: "archivo\d" coincide con "archivo1".

advanced-regex-word-char = Caracteres de palabra

advanced-regex-word-char-desc = Coincide con letras, números y guiones bajos.

advanced-regex-start-line = Inicio de línea

advanced-regex-start-line-desc = Coincide solo al principio del nombre.

advanced-regex-end-line = Fin de línea

advanced-regex-end-line-desc = Coincide solo al final del nombre.

advanced-regex-one-or-more = Uno o más

advanced-regex-one-or-more-desc = Coincide con uno o más del carácter anterior.

advanced-regex-optional = Opcional

advanced-regex-optional-desc = Coincide con cero o uno del carácter anterior.

advanced-regex-either-or = Uno u otro

advanced-regex-either-or-desc = Coincide con la parte izquierda o derecha.

advanced-regex-range = Rango de caracteres

advanced-regex-range-desc = Coincide con cualquier carácter dentro de los corchetes.

advanced-regex-negated-set = Conjunto negado

advanced-regex-negated-set-desc = Coincide con cualquier carácter QUE NO esté en los corchetes.

advanced-regex-group = Grupo

advanced-regex-group-desc = Agrupa múltiples símbolos.

advanced-regex-count-exact = Recuento exacto

advanced-regex-count-exact-desc = Coincide exactamente n veces.

advanced-size = Tamaño

advanced-date = Modificado

advanced-min = Mín

advanced-max = Máx

advanced-invalid = Comprueba los patrones, tamaños y fechas (AAAA-MM-DD).

advanced-date-placeholder = AAAA-MM-DD

advanced-weekdays = Lun Mar Mié Jue Vie Sáb Dom

duplicate-locations = Ubicaciones de búsqueda

duplicate-settings = Opciones de búsqueda

duplicate-name = Nombre

duplicate-size = Tamaño

duplicate-content = Contenido (lento)

duplicate-current = Carpeta actual

duplicate-warning = Seleccionar varias ubicaciones puede ralentizar considerablemente la búsqueda.

duplicate-filter = Filtrar resultados...

duplicate-idle = Configura la búsqueda y haz clic en Buscar.

duplicate-empty = No se encontraron duplicados.

duplicate-scanning = Analizando archivos: { $current }

duplicate-hashing = Comparando contenido: { $current } / { $total }

duplicate-groups = { $groups ->
    [one] { $group_count } grupo de duplicados
   *[other] { $group_count } grupos de duplicados
    }

duplicate-files = { $file_count ->
    [one] { $file_number } archivo
   *[other] { $file_number } archivos
    }

duplicate-group = Grupo { $index }

duplicate-failed = La búsqueda ha fallado: { $error }

duplicate-more = Cargar más

duplicate-disk = Disco n.º { $disk }

duplicate-go-to-folder = Haz clic para ir a la carpeta

folder-size-empty = Vacío

quick-search-current-folder = Carpeta actual

settings-clear-cache-confirm = ¿Eliminar las cachés de iconos y miniaturas? Se volverán a crear cuando sea necesario.

settings-reset-confirm = ¿Restablecer las preferencias predeterminadas? Se conservarán las pestañas abiertas.

settings-cache-clearing = Borrando caché…

settings-cache-cleared = Caché borrada.

settings-cache-error = No se pudo borrar toda la caché. Inténtalo de nuevo.

settings-preferences-reset = Preferencias restablecidas.

settings-turbo-description = Modo Turbo predeterminado para nuevas copias y movimientos. Las transferencias en curso mantienen su configuración.

settings-search-limit-description = Máximo de resultados para nuevas búsquedas rápidas y avanzadas. Pasos de 500, mínimo 500.

navigation-up = Subir

about-releases = Ver versiones
about-repository = Repositorio de GitHub
about-main-tools = Herramientas principales

settings-search-limit-reset = Restablecer el límite predeterminado de 3.000 resultados

menu-copy-name = Copiar nombre

trash-restore = Restaurar
trash-restore-all = Restaurar todo
trash-delete-permanent = Eliminar permanentemente
trash-delete-all = Vaciar la papelera
trash-all = Todo
trash-original-location = Ubicación original
trash-deleted-at = Fecha de eliminación

trash-restore-selection = Restaurar la selección

original-location-filter-placeholder = Filtrar por ubicación original…

trash-properties = Propiedades de la Papelera
trash-locations = Ubicación de la Papelera
trash-available-space = Espacio disponible
trash-selected-settings = Configuración de la ubicación seleccionada
trash-custom-size = Tamaño personalizado
trash-max-capacity = Tamaño máximo
trash-dont-move = No mover archivos a la Papelera
trash-delete-immediately = Eliminar los archivos inmediatamente
trash-confirm-delete = Mostrar confirmación de eliminación
trash-megabytes = MB
trash-settings-error = No se pudo aplicar la configuración. Compruebe la capacidad.

trash-settings-ok = OK
trash-delete-prompt = ¿Eliminar los elementos seleccionados?

trash-item-count = { $count ->
    [one] ({ $formatted } elemento)
   *[other] ({ $formatted } elementos)
    }

trash-permanent-delete-prompt = ¿Eliminar definitivamente los elementos seleccionados?

drive-tooltip-usage = { $used } / { $total }

properties-system = Propiedades del sistema
properties-created = Creado
properties-accessed = Accedido
properties-used = Espacio utilizado
properties-capacity = Capacidad
properties-contains = Contenido
properties-counts = { $files } archivos, { $folders } carpetas
properties-drive = Unidad

properties-copy-name-double-click = Doble clic para copiar el nombre
