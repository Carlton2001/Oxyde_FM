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

conflict-skip-all = Omitir todos

conflict-replace-all = Reemplazar todo

common-close = Cerrar

common-retry = Reintentar

settings-title = Preferencias

settings-appearance = Apariencia

settings-theme = Tema

settings-scale = Escala de la interfaz

settings-scale-value = { $percentage } %

settings-scale-reset = Restablecer la escala al 100 %

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
tooltip-original-location = Ubicación original: { $value }
tooltip-deleted = Eliminado: { $value }

menu-open = Abrir

menu-open-new-tab = Abrir en nueva pestaña
menu-open-terminal = Abrir en la consola

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
menu-deselect-all = Deseleccionar todo

menu-select-all-checks = Marcar todos

menu-deselect-all-checks = Desmarcar todos

column-name = Nombre

column-type = Tipo

column-size = Tamaño

column-modified = Modificado

file-type-folder = Carpeta

file-type-file = Archivo

file-type-symlink = Enlace simbólico
file-type-broken-link = Enlace roto

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


operations-title = { $count ->
    [1] Operación ({ $count })
   *[other] Operaciones ({ $count })
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


operations-to = Hasta: { $path }

operations-items = { $count ->
    [1] { $done } / { $total } elemento
   *[other] { $done } / { $total } elementos
    }

operations-remaining = Tiempo restante: { $seconds } s

operations-duration = Duración: { $duration }

operations-average = Promedio: { $speed }

operations-speed-waiting = Esperando...
operations-finalizing = Finalizando…

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



sidebar-quick-access = Acceso rápido

sidebar-drives = Unidades

sidebar-locations = UBICACIONES

sidebar-network = Red

sidebar-trash = Papelera
sidebar-home = Carpeta personal

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

path-error-access-denied = Carpeta protegida (Se requieren derechos de administrador)

path-error-failed = No se pudo abrir esta carpeta. Comprueba la ruta e inténtalo de nuevo.

undo-failed = No se pudo deshacer: { $detail }
undo-copy-identity-changed = Este elemento ya no corresponde al objeto copiado originalmente: { $detail }. Se detuvo Deshacer para conservar sus archivos.
undo-copy-unverified = El historial no permite verificar la identidad de esta copia: { $detail }. Se han conservado sus archivos.

undo-overwrite-unavailable = Esta copia no se puede deshacer porque ha reemplazado archivos existentes.

redo-failed = No se pudo rehacer: { $detail }
redo-copy-source-changed = El origen ha cambiado desde la copia: { $detail }. Se detuvo Rehacer para conservar sus archivos.
redo-copy-unverified = El historial no permite verificar esta copia: { $detail }. Inicie una nueva copia desde la carpeta de origen.

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
unlabeled-volume = Volumen de { $size }

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

settings-turbo = Modo Turbo

settings-search-limit = Límite de búsqueda

settings-data = Datos de la aplicación

settings-clear-cache = Borrar caché

settings-reset-preferences = Restablecer preferencias

title-new-panel = Nuevo panel a la derecha

title-tools = Búsqueda

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


date-group-count = { $count ->
    [one] { $number } elemento
   *[other] { $number } elementos
    }

menu-sort-by = Ordenar por

menu-sort-ascending = Ascendente

menu-sort-descending = Descendente

menu-sort-extension = Extensión

drive-eject = Expulsar
drive-unmount = Desmontar

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
remote-login-title = Se necesita autenticación
remote-domain = Dominio
remote-anonymous = Conectar de forma anónima
remote-remember-never = Olvidar la contraseña inmediatamente
remote-remember-session = Recordar la contraseña hasta que cierre la sesión
remote-remember-always = Recordar siempre
remote-retry = Credenciales rechazadas. Inténtelo de nuevo.
remote-connect = Conectar
remote-connect-failed = No se puede conectar con esta ubicación.
remote-connect-failed-toast = No se puede conectar: { $detail }

drive-eject-failed = No se pudo expulsar la unidad. Puede que otra aplicación la esté usando. { $detail }
drive-unmount-failed = No se pudo desmontar el volumen. Puede que otra aplicación lo esté usando. { $detail }
drive-holders = Abierto en: { $applications }.
drive-mount-failed = No se pudo montar el volumen. { $detail }
drive-mount-hibernated = Windows no se apagó por completo (hibernación o inicio rápido), así que este volumen sigue bloqueado. Apague Windows por completo y vuelva a intentarlo.

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

duplicate-size-range = { $min } – { $max }

duplicate-disk = Disco n.º { $disk }

duplicate-go-to-folder = Haz clic para ir a la carpeta

folder-size-empty = Vacío
folder-size-provisional-description =
    Tamaño pendiente de actualizar.
    El recálculo automático espera a que disminuya la actividad.
    Actualice para forzar la actualización.

quick-search-current-folder = Carpeta actual

settings-clear-cache-confirm = ¿Eliminar las cachés de iconos y miniaturas? Se volverán a crear cuando sea necesario.

settings-reset-confirm = ¿Restablecer las preferencias predeterminadas? Se conservarán las pestañas abiertas.

settings-cache-clearing = Borrando caché…

settings-cache-cleared = Caché borrada.

settings-cache-error = No se pudo borrar toda la caché. Inténtalo de nuevo.

settings-preferences-reset = Preferencias restablecidas.

settings-turbo-description = Modo Turbo predeterminado para nuevas copias, movimientos, eliminaciones, envíos a la Papelera, compresiones y extracciones. También se aplica a los cálculos de tamaño de carpetas, incluidos los que están en curso. Las operaciones de archivos en curso mantienen su configuración individual.

settings-search-limit-description = Máximo de resultados para nuevas búsquedas rápidas y avanzadas. Pasos de 500, mínimo 500.

navigation-up = Subir

about-repository = Repositorio de GitHub
about-main-tools = Herramientas principales
update-available-toast = Oxyde { $detail } está disponible. Instálala desde Ajustes › Acerca de.
update-failed-toast = No se pudo actualizar: { $detail }
update-check = Buscar actualizaciones
update-checking = Buscando…
update-current = Oxyde está actualizado
update-check-failed = Error en la búsqueda, reintentar
update-install = Instalar la versión { $version }
update-download-portable = Descargar la versión { $version }
update-downloading = Descargando la actualización…

settings-search-limit-reset = Restablecer el límite predeterminado de 3.000 resultados

menu-copy-name = Copiar nombre

trash-restore = Restaurar
trash-restore-all = Restaurar todo
trash-delete-permanent = Eliminar permanentemente
trash-delete-all = Vaciar la papelera
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
trash-auto-delete = Eliminación automática
trash-auto-delete-old = Eliminar automáticamente los elementos antiguos
trash-retention-days = Conservar los elementos (días)
trash-gnome-retention-note = Ajuste de GNOME, aplicado sea cual sea la aplicación que eliminó los archivos.
trash-kde-time-limit = Eliminar los elementos con más de (días)
trash-kde-size-limit = Limitar el tamaño (% del disco)
trash-kde-limit-action = Al alcanzar el límite
trash-kde-warn = Avisar
trash-kde-delete-oldest = Eliminar los más antiguos
trash-kde-delete-largest = Eliminar los más grandes
trash-kde-retention-note = Ajustes de KDE, aplicados a los archivos eliminados desde Dolphin y las aplicaciones de KDE.
trash-retention-error = No se pudo aplicar la configuración. Compruebe los valores introducidos.

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
properties-link-target = Destino del enlace
properties-used = Espacio utilizado
properties-capacity = Capacidad
properties-contains = Contenido
properties-counts = { $files } archivos, { $folders } carpetas
properties-drive = Unidad
properties-file-system = Sistema de archivos

properties-copy-name-double-click = Doble clic para copiar el nombre

toast-copied = Copiado al portapapeles
toast-cut = Cortado al portapapeles
toast-names-copied = { $count ->
    [one] { $number } nombre copiado
   *[other] { $number } nombres copiados
    }
toast-paths-copied = { $count ->
    [one] { $number } ruta copiada
   *[other] { $number } rutas copiadas
    }
toast-restored = { $count ->
    [one] { $number } elemento restaurado
   *[other] { $number } elementos restaurados
    }
toast-purged = { $count ->
    [one] { $number } elemento eliminado permanentemente
   *[other] { $number } elementos eliminados permanentemente
    }
toast-mounted = Imagen de disco montada
toast-unmounted = Imagen de disco desmontada

toast-undo = Operación deshecha
toast-redo = Operación rehecha

toast-failed = La operación falló: { $detail }

toast-copy-completed = Copia completada
toast-move-completed = Movimiento completado
toast-trash-completed = Enviado a la papelera
toast-delete-completed = Eliminación permanente completada
toast-compress-completed = Archivo comprimido creado
toast-extract-completed = Extracción completada

conflict-source = Archivo nuevo
conflict-target = Archivo existente
conflict-newer = Más reciente
conflict-older = Más antiguo
conflict-larger = Más grande
conflict-items = elementos
conflict-skip = Omitir
conflict-replace = Reemplazar
conflict-all = Todos
conflict-identical = Idénticos
conflict-search = Filtrar resultados…
conflict-empty = No hay conflictos coincidentes
conflict-prompt = Ya existe un elemento llamado «{ $name }». ¿Qué desea hacer?
conflict-advanced-title = Resolución de conflictos
conflict-advanced = Avanzado
conflict-apply = Aplicar decisiones
conflict-step-title = { $current } / { $total } Conflicto de archivo
conflict-inherited = Heredado de { $parent }: { $action }
conflict-back = Volver
conflict-recent-tab = Recientes
conflict-older-tab = Antiguos
conflict-automatic = Auto
conflict-apply-summary = Aplicar: { $replace_count ->
        [one] { $replace_number } reemplazo
       *[other] { $replace_number } reemplazos
    }, { $skip_count ->
        [one] { $skip_number } omitido
       *[other] { $skip_number } omitidos
    }

peek-failed = No se pudo abrir Peek
properties-general = General
properties-shortcut = Acceso directo
shortcut-target = Destino
shortcut-arguments = Argumentos
shortcut-working-directory = Iniciar en
shortcut-comment = Comentario
shortcut-run = Ejecutar
shortcut-normal = Ventana normal
shortcut-minimized = Minimizada
shortcut-maximized = Maximizada
shortcut-saving = Guardando…
shortcut-invalid-target = El destino no puede estar vacío y los campos deben ser válidos.

# Archive cache recovery and resource limits
archive-cache-full = No se puede abrir { $detail }. Oxyde no puede liberar suficiente espacio: hay copias en uso, modificadas o recién abiertas. Cierre los documentos que no utilice y vuelva a intentarlo, o extraiga este archivo a una carpeta de su elección.
archive-cache-busy = No se puede abrir { $detail }. Todavía se está preparando la apertura de otro archivo comprimido. Inténtelo de nuevo en un momento, o extraiga este archivo a una carpeta de su elección.
archive-cache-unavailable = No se puede abrir { $detail }. Oxyde no pudo preparar la copia temporal. Puede extraer este archivo a una carpeta de su elección.
archive-cache-extract = Extraer y abrir…
archive-cache-exported = Archivo extraído: { $detail }
archive-limit-bytes = Procesamiento detenido: el tamaño total descomprimido supera el límite de { $limit } para esta acción. No se toma en cuenta el tamaño comprimido del archivo.
archive-processing-failed = No se pudo procesar el archivo. Puede estar dañado o necesitar demasiados recursos.
archive-recovery-notice = Archivos conservados
archive-recovery-completed = La acción ha terminado, pero se han conservado algunos archivos o carpetas. Compruebe estas ubicaciones antes de eliminarlos: { $locations }
archive-recovery-stopped = La acción no se ha completado. Se han conservado algunos archivos o carpetas, incluidos resultados ya publicados. Compruebe estas ubicaciones antes de volver a intentarlo: { $locations }
archive-recovery-more = Hay otras { $count } ubicaciones en el registro de la aplicación.
archive-limit-source-bytes = Creación del archivo comprimido detenida: el tamaño total de los archivos de origen supera el límite de { $limit } para esta acción.
archive-limit-encoded-bytes = Creación del archivo comprimido detenida: el tamaño de salida supera el límite de { $limit } para esta acción.
archive-limit-entries = Procesamiento detenido: el archivo comprimido contiene más de { $limit } archivos y carpetas.
archive-limit-depth = Procesamiento detenido: una ruta del archivo comprimido supera { $limit } niveles, incluido el nombre del archivo.
archive-limit-time = Procesamiento detenido: se alcanzó el tiempo máximo de procesamiento activo de { $limit } minutos. Las pausas no cuentan.
archive-limit-decoded = Procesamiento detenido: los datos descomprimidos, incluidas las cabeceras, superan { $limit }.
archive-source-changed = El archivo comprimido cambió durante la operación. Oxyde detuvo la operación y conservó la versión actual del archivo. Repita la operación con esta versión.

quick-search-archives-skipped =
    { $count ->
        [one] Resultados incompletos: { $number } archivo comprimido omitido
       *[other] Resultados incompletos: { $number } archivos comprimidos omitidos
    }
quick-search-archive-skipped-detail = Primer archivo comprimido omitido: { $path }. { $reason }
quick-search-archive-unreadable = Este archivo comprimido no es válido, no es compatible o no se puede leer.
quick-search-archive-content-not-searched = No se busca en el contenido de los archivos comprimidos. Las entradas de este archivo se excluyen mientras haya un filtro de contenido activo.
quick-search-archive-dates-unavailable = Las fechas de las entradas no están disponibles para este formato de archivo comprimido. Sus entradas se excluyen mientras haya un filtro de fecha activo.

undo-new-folder-changed = La carpeta ya no corresponde a la que se creó: { $detail }. Se detuvo Deshacer para conservar sus archivos.
undo-new-folder-unverified = El historial no permite verificar esta carpeta: { $detail }. Se conservan la carpeta y su contenido.
undo-new-folder-not-empty = La carpeta ahora contiene archivos o subcarpetas: { $detail }. Deshacer no elimina su contenido.
undo-new-folder-missing = No se encuentra la carpeta creada: { $detail }. Se detuvo Deshacer sin eliminar otros elementos.
redo-new-folder-conflict = Ya existe un elemento en esta ubicación: { $detail }. Rehacer no lo reemplaza.
redo-new-folder-parent-missing = No se encuentra la carpeta superior: { $detail }. Rehacer no vuelve a crear las carpetas superiores.
redo-new-folder-unverified = El historial no permite rehacer la creación de esta carpeta: { $detail }. No se ha creado ningún elemento.

redo-move-changed = El archivo o la carpeta ha cambiado desde la operación: { $detail }. Se ha detenido Rehacer el movimiento para conservar sus archivos.
redo-move-conflict = Un elemento ocupa el destino: { $detail }. Rehacer el movimiento no lo reemplaza.
redo-move-unverified = Este historial no permite rehacer el movimiento de forma segura: { $detail }. Inicie el movimiento de nuevo desde los archivos actuales.
redo-move-incomplete = El movimiento se detuvo y algunos elementos no pudieron volver a su ubicación original. Se han conservado archivos o carpetas para su recuperación: { $detail }. Compruebe estas ubicaciones antes de volver a intentarlo.

undo-recovery-incomplete = Se ha detenido la acción de deshacer y algunos elementos no pudieron volver a su ubicación original. Se han conservado archivos o carpetas para su recuperación. Compruebe estas ubicaciones antes de volver a intentarlo: { $detail }.
preview-unavailable = No se pudo cargar esta vista previa. Intente abrir el archivo directamente.
preview-image-limit = Esta imagen supera los límites de la vista previa. El archivo sigue siendo accesible.
preview-animation-static = Vista previa estática: la animación supera los límites de la vista previa.

undo-move-changed = No se puede deshacer este movimiento: el archivo o la carpeta fue reemplazado o creado de nuevo. Se ha conservado en su ubicación actual. { $detail }
undo-move-unverified = No se puede deshacer este movimiento de forma segura: no se pudo verificar la identidad del archivo o la carpeta. Se ha conservado. { $detail }

undo-rename-changed = No se puede deshacer este cambio de nombre: el archivo o la carpeta fue reemplazado. Se ha conservado en su ubicación actual. { $detail }
undo-rename-unverified = No se puede deshacer este cambio de nombre de forma segura: no se pudo verificar la identidad del archivo o la carpeta. Se ha conservado. { $detail }
redo-rename-changed = No se puede rehacer este cambio de nombre: el archivo o la carpeta fue reemplazado. Se ha conservado en su ubicación actual. { $detail }
redo-rename-unverified = No se puede rehacer este cambio de nombre de forma segura: no se pudo verificar la identidad del archivo o la carpeta. Se ha conservado. { $detail }

transfer-source-changed = El origen se ha sustituido o ya no es un archivo o carpeta normal. La operación se ha detenido; los elementos ya transferidos se conservan. Compruebe el origen e inténtelo de nuevo.
transfer-source-unverified = No se puede verificar la identidad del origen en esta ubicación. La operación se ha detenido; los elementos ya transferidos se conservan. Pruebe desde una carpeta local compatible.

trash-restore-ambiguous = Varios elementos de la papelera proceden de la misma ubicación. Abra la papelera y seleccione el elemento concreto que desea restaurar. Los elementos ya restaurados se conservan.
invalid-encoding-name = { $name } (codificación no válida)
unrepresentable-path = El nombre de «{ $path }» no es un texto UTF-8 válido. Cámbiele el nombre antes de copiarlo, moverlo, eliminarlo o comprimirlo; no se ha modificado nada.
properties-permissions = Permisos
properties-contents = Contenido
permissions-owner = Propietario
permissions-group = Grupo
permissions-others = Otros
permissions-access = Acceso
permissions-execution = Ejecutar
permissions-executable = Permitir ejecutar el archivo como un programa
permissions-none = Ninguno
permissions-read-only = Solo lectura
permissions-read-write = Lectura y escritura
permissions-list-files = Solo listar archivos
permissions-access-files = Acceder a los archivos
permissions-create-delete = Crear y eliminar archivos
permissions-not-owner = No es el propietario, por lo que no puede cambiar estos permisos.
permissions-changed = El elemento ha cambiado desde que se abrió esta ventana; no se ha modificado nada. Vuelva a abrir sus propiedades.
permissions-not-kept = Este sistema de archivos no conserva los permisos Unix (NTFS o FAT, por ejemplo): se han quedado como estaban.
permissions-unchanged = Sin cambios
permissions-enclosed = Cambiar los permisos de los elementos contenidos
permissions-enclosed-hint = Se aplica a todos los archivos y carpetas contenidos, a cualquier profundidad, al pulsar Aceptar. Los enlaces simbólicos y los elementos de otros discos no se modifican.
permissions-files = Archivos
permissions-folders = Carpetas
permissions-incomplete = No se han podido modificar { $count } elementos contenidos; los demás y la carpeta sí. Primer fallo: { $detail }
permissions-multiple = Varios
permissions-mixed = Mixto
permissions-folder-access = Acceso a carpetas
permissions-file-access = Acceso a archivos
permissions-selection-incomplete = No se han podido modificar { $count } elementos; los demás sí. Primer fallo: { $detail }
trash-restore-changed = El elemento de la papelera cambió después de seleccionarlo. Se detuvo la restauración. Actualice la papelera y vuelva a seleccionar el elemento. Los elementos ya restaurados se conservan.
trash-restore-conflict = Ya existe un archivo o carpeta en la ubicación de restauración. Se conserva y el elemento afectado sigue en la papelera. Cambie el nombre o mueva el destino e inténtelo de nuevo.
trash-restore-unavailable = No se pudo restaurar este elemento. Compruebe que sigue en la papelera y que su carpeta original existe y es accesible. Inténtelo de nuevo. Los elementos ya restaurados se conservan.

history-delete-changed = El elemento ya no coincide con el registrado al eliminarlo o no está disponible. Se detuvo Deshacer o Rehacer para conservar los archivos actuales. Los elementos ya procesados se conservan.
history-delete-unverified = Oxyde no tiene la información necesaria para deshacer o rehacer esta eliminación de forma segura. Se detuvo la acción. Puede comprobar la papelera y restaurar manualmente el elemento deseado.

preview-timed-out = Esta vista previa ha superado el límite de 15 segundos. El archivo sigue siendo accesible.
copy-name-suffix = Copia
shell-delete-completed-error = Windows terminó de eliminar este elemento y después notificó un error. La operación completada se conserva en el historial. Compruebe la carpeta y la papelera antes de intentarlo de nuevo.
shell-delete-unconfirmed = Windows no confirmó la eliminación de este elemento. La eliminación puede haber sido parcial. Compruebe la carpeta y la papelera antes de intentarlo de nuevo.

archive-listing-metadata-limit = No se puede mostrar el contenido: la lista de archivos supera { $limit }. Este límite se aplica a los nombres y a la información de los archivos, no a su tamaño descomprimido. Puede intentar extraerlos con los límites habituales.
archive-listing-worker-failed = No se puede leer el contenido de este archivo: el proceso de lectura ha fallado o su respuesta no es válida. El archivo no se ha modificado. Puede volver a intentarlo o extraerlo con los límites habituales.

toast-transfer-skipped = Se omitieron todos los elementos.

properties-calculating-size = Calculando el tamaño…
drag-new-tab = Crear una nueva pestaña
drag-unavailable = No se puede soltar aquí
properties-loading = Cargando las propiedades…

storage-unavailable = No se pudo acceder a los datos de Oxyde. En modo portátil, la carpeta data debe permitir la escritura.

properties-selection-items = { $number } elementos
properties-total-size = Tamaño total
properties-multiple-locations = Varias ubicaciones
operations-affected-items = { $count ->
    [1] Elemento de la operación ({ $number })
   *[other] Elementos de la operación ({ $number })
    }
operations-view-items = Ver los { $count } elementos…
operations-items-filter = Buscar nombres o rutas…
operations-no-matching-items = No hay elementos coincidentes.
operations-items-loading = Cargando elementos…
operations-item-done = Procesado
operations-item-not-processed = Sin procesar
operations-item-skipped = Omitido
operations-item-partial = Procesado parcialmente
operations-item-unknown = Estado desconocido
operations-item-interrupted = Interrumpido

default-archive-format = Formato de archivo predeterminado

settings-onedrive = OneDrive
settings-onedrive-description = Mostrar OneDrive en la barra lateral cuando esté configurado.

trash-restore-summary = Restauración terminada: { $restored } elemento(s) restaurado(s), { $failed } fallido(s). Los demás elementos se procesaron. Consulte el registro para más detalles.
transfer-destination-collision = Varios elementos seleccionados tendrían el mismo destino: { $path }. Cámbieles el nombre o transfiéralos por separado. No se ha transferido ningún archivo.
archive-link-refused = Un archivo comprimido no puede contener un enlace simbólico: { $path }. Oxyde rechaza los enlaces al abrir y extraer archivos comprimidos, para que ninguna extracción pueda escribir fuera de su destino. No se ha modificado nada.

grouping = Agrupar por
group-none = Ninguno
group-date = Fecha de modificación
group-type = Tipo de archivo
group-folders = Carpetas
group-archives = Archivos comprimidos
group-applications = Aplicaciones e instaladores
group-documents = Documentos
group-images = Imágenes
group-video = Vídeos
group-audio = Audio
group-code = Código y scripts
group-other = Otros
group-disk-images = Imágenes de disco
mouse-right-click = Clic derecho
grouping-current = Agrupado por: { $mode }
group-size = Tamaño
group-extension = Extensión
group-created = Fecha de creación
group-deleted = Fecha de eliminación
group-size-empty = Vacío (0 bytes)
group-no-extension = Sin extensión
group-origin-unknown = Ubicación original desconocida
group-date-unknown = Fecha desconocida
group-select-all = Seleccionar todos los elementos del grupo
group-deselect-all = Deseleccionar todos los elementos del grupo

viewer-title = Visor
viewer-visual-paused = Visualización del vídeo en pausa
viewer-file-unavailable = No se puede mostrar este archivo.
viewer-open = Abrir el Visor
viewer-close = Cerrar el Visor
viewer-slide = Diapositiva
viewer-text-truncated = Solo se muestran los primeros 512 MiB del archivo.
viewer-empty = Seleccione un archivo en un panel del explorador.
viewer-detach = Separar el Visor
viewer-detached = El Visor está abierto en una ventana secundaria.
viewer-show-window = Mostrar la ventana del Visor
viewer-play = Reproducir
viewer-pause = Pausa

viewer-rendered = Renderizado
viewer-code = Código
viewer-highlighted = Resaltado de sintaxis

viewer-enable-highlighting = Activar el resaltado de sintaxis
viewer-disable-highlighting = Desactivar el resaltado de sintaxis

group-scripts = Scripts

viewer-loading = Cargando archivo…
viewer-autoplay = Reproducción automática
viewer-enable-autoplay = Activar la reproducción automática de los próximos archivos multimedia
viewer-disable-autoplay = Desactivar la reproducción automática de los próximos archivos multimedia

audio-no-tags = No hay etiquetas disponibles
audio-album = Álbum
audio-track = Pista
audio-year = Año


quick-search-content-skipped =
    { $count ->
        [one] Contenido sin verificar: { $number } archivo
       *[other] Contenido sin verificar: { $number } archivos
    }
quick-search-content-skipped-detail = Búsqueda de contenido incompleta en { $path }: documento ilegible o límite de recursos alcanzado.
