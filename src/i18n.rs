//! Interface translations. The app is written in English; each translation maps an
//! exact English string to its Spanish, German, or French text. A string with no
//! entry is shown in English, so missing translations never break the interface.
//! The language follows the operating system (`LC_ALL`, `LC_MESSAGES`, `LANGUAGE`, `LANG`).
//! Only static labels are translated; status messages and file names are not.
use std::{
    collections::HashMap,
    sync::{
        LazyLock,
        atomic::{AtomicU8, Ordering},
    },
};

/// The languages the interface is translated into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolved {
    English = 0,
    Spanish = 1,
    German = 2,
    French = 3,
}

static ACTIVE: AtomicU8 = AtomicU8::new(0);

/// Pick a language from a locale such as `es_MX.UTF-8` or a `LANGUAGE` list like `fr:en`.
pub fn detect(locale: &str) -> Resolved {
    let first = locale.split(':').next().unwrap_or_default();
    match first.get(..2).map(str::to_ascii_lowercase).as_deref() {
        Some("es") => Resolved::Spanish,
        Some("de") => Resolved::German,
        Some("fr") => Resolved::French,
        _ => Resolved::English,
    }
}

/// The first usable locale variable. Snap and desktop sessions pass these through.
#[cfg(unix)]
fn system_locale() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty() && value != "C" && value != "POSIX")
        .unwrap_or_default()
}

#[cfg(windows)]
fn system_locale() -> String {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
    let mut locale = [0; 85];
    let len = unsafe { GetUserDefaultLocaleName(locale.as_mut_ptr(), locale.len() as i32) };
    String::from_utf16_lossy(&locale[..len.saturating_sub(1).max(0) as usize])
}

/// Use the system language when it is translated, and English otherwise. There is no
/// in-app language choice: the language always follows the operating system.
pub fn init() {
    ACTIVE.store(detect(&system_locale()) as u8, Ordering::Relaxed);
}

/// Used by the screenshot tool to render a specific language.
pub fn set_resolved(language: Resolved) {
    ACTIVE.store(language as u8, Ordering::Relaxed);
}

fn lookup(table: &'static [(&'static str, &'static str)]) -> HashMap<&'static str, &'static str> {
    table.iter().copied().collect()
}
static SPANISH: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| lookup(ES));
static GERMAN: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| lookup(DE));
static FRENCH: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| lookup(FR));

pub fn translate(language: Resolved, text: &str) -> &str {
    let table = match language {
        Resolved::English => return text,
        Resolved::Spanish => &*SPANISH,
        Resolved::German => &*GERMAN,
        Resolved::French => &*FRENCH,
    };
    table.get(text).copied().unwrap_or(text)
}

/// The text in the active language, or unchanged English when there is no translation.
pub fn tr(text: &str) -> &str {
    let language = match ACTIVE.load(Ordering::Relaxed) {
        1 => Resolved::Spanish,
        2 => Resolved::German,
        3 => Resolved::French,
        _ => Resolved::English,
    };
    translate(language, text)
}

const ES: &[(&str, &str)] = &[
    ("No matches", "Sin coincidencias"),
    ("Open ThermalPrint", "Abrir ThermalPrint"),
    ("Check barcodes", "Comprobar códigos"),
    ("Background printing", "Impresión en segundo plano"),
    (
        "Keep running when the window is closed",
        "Seguir ejecutándose al cerrar la ventana",
    ),
    (
        "The tray menu opens ThermalPrint, pauses automatic printing, or quits. Your computer must be awake.",
        "El menú de la bandeja permite abrir ThermalPrint, pausar la impresión automática o salir. El equipo debe estar activo.",
    ),
    ("Start at login", "Iniciar al iniciar sesión"),
    (
        "Automatic printing checks file contents to prevent repeat prints, even after a file is renamed.",
        "La impresión automática comprueba el contenido para evitar duplicados, incluso si se cambia el nombre del archivo.",
    ),
    (
        "Clear duplicate protection…",
        "Borrar protección contra duplicados…",
    ),
    (
        "Clear duplicate protection?",
        "¿Borrar protección contra duplicados?",
    ),
    (
        "Previously printed files can print automatically again if they arrive in the watch folder.",
        "Los archivos ya impresos podrán volver a imprimirse automáticamente si llegan a la carpeta supervisada.",
    ),
    (
        "Clear duplicate protection",
        "Borrar protección contra duplicados",
    ),
    (
        "Printer setup assistant",
        "Asistente de configuración de impresora",
    ),
    (
        "Set up printer and label stock",
        "Configurar impresora y etiquetas",
    ),
    ("Next", "Siguiente"),
    ("Print one test label", "Imprimir una etiqueta de prueba"),
    ("Close assistant", "Cerrar asistente"),
    ("Saved label stock", "Etiquetas guardadas"),
    ("Stock profile name", "Nombre del perfil de etiquetas"),
    ("Save stock profile", "Guardar perfil de etiquetas"),
    ("Reprint…", "Reimprimir…"),
    (
        "Review and reprint with the recorded settings",
        "Revisar y reimprimir con los ajustes guardados",
    ),
    ("Reprint label?", "¿Reimprimir etiqueta?"),
    (
        "This sends another print job using the recorded settings.",
        "Esto envía otro trabajo con los ajustes guardados.",
    ),
    ("Reprint", "Reimprimir"),
    ("Cable flag", "Bandera para cable"),
    (
        "Matching cable IDs on both sides of a fold",
        "Identificadores iguales a ambos lados del pliegue",
    ),
    ("Price and barcode", "Precio y código de barras"),
    (
        "Product, price, and scannable stock code",
        "Producto, precio y código de inventario",
    ),
    ("Shelf barcode", "Código de estante"),
    (
        "Shelf location with a stock barcode",
        "Ubicación de estante con código de inventario",
    ),
    ("Small product QR", "QR de producto pequeño"),
    (
        "Compact product details and QR code",
        "Datos compactos del producto y código QR",
    ),
    ("Printing", "Imprimiendo"),
    (
        "Print labels automatically?",
        "¿Imprimir etiquetas automáticamente?",
    ),
    (
        "Every label that lands in the watch folder will print by itself, without you pressing Print. A folder used only for labels is safest.",
        "Cada etiqueta que llegue a la carpeta vigilada se imprimirá sola, sin que pulses Imprimir. Lo más seguro es una carpeta usada solo para etiquetas.",
    ),
    (
        "This looks like your Downloads folder: an invoice or any other PDF saved there could print too. A separate folder just for labels is safer.",
        "Parece tu carpeta de Descargas: una factura o cualquier otro PDF guardado allí también podría imprimirse. Es más seguro usar una carpeta aparte solo para etiquetas.",
    ),
    (
        "Only single-page labels whose crop needs no review print. Each one waits 5 seconds with a Cancel button, and Pause stops everything.",
        "Solo se imprimen las etiquetas de una página cuyo recorte no necesita revisión. Cada una espera 5 segundos con un botón Cancelar, y Pausar lo detiene todo.",
    ),
    ("Turn on", "Activar"),
    ("auto-print paused", "impresión automática en pausa"),
    ("auto-print on", "impresión automática activada"),
    ("Resume auto-print", "Reanudar impresión automática"),
    ("Pause auto-print", "Pausar impresión automática"),
    (
        "Stop or restart printing labels by themselves. Waiting labels stay in the queue.",
        "Detiene o reanuda la impresión automática de etiquetas. Las etiquetas en espera permanecen en la cola.",
    ),
    (
        "Print labels automatically",
        "Imprimir etiquetas automáticamente",
    ),
    (
        "Print labels that land in this folder by themselves. Only single-page labels whose crop needs no review print; the rest wait in the queue. Off until you turn it on.",
        "Imprime solas las etiquetas que llegan a esta carpeta. Solo se imprimen las de una página cuyo recorte no necesita revisión; las demás esperan en la cola. Desactivado hasta que lo actives.",
    ),
    ("Only files named", "Solo archivos llamados"),
    (
        "Leave empty to print every label. Use * for any letters and ? for one letter; separate several names with commas.",
        "Déjalo vacío para imprimir todas las etiquetas. Usa * para cualquier grupo de letras y ? para una letra; separa varios nombres con comas.",
    ),
    (
        "File names that print automatically",
        "Nombres de archivo que se imprimen automáticamente",
    ),
    ("Most per minute", "Máximo por minuto"),
    (
        "A burst of files cannot print more labels than this in one minute; the rest wait.",
        "Una ráfaga de archivos no puede imprimir más etiquetas que esto en un minuto; el resto espera.",
    ),
    (
        "Most labels printed automatically per minute",
        "Máximo de etiquetas impresas automáticamente por minuto",
    ),
    ("Safe margin", "Margen de seguridad"),
    (
        "Keep content this far from the label's edge. It is shaded on the canvas and objects inside it are flagged, even when the printer's driver reports no unprintable border. It is remembered for each printer.",
        "Mantén el contenido a esta distancia del borde de la etiqueta. Se sombrea en el lienzo y se señalan los objetos dentro de él, aunque el controlador de la impresora no indique ningún borde no imprimible. Se recuerda para cada impresora.",
    ),
    (
        "Safe margin in millimeters",
        "Margen de seguridad en milímetros",
    ),
    (
        "These objects reach into the unprintable border or your safe margin and may be cut off:",
        "Estos objetos llegan al borde no imprimible o a tu margen de seguridad y pueden recortarse:",
    ),
    (
        "Choose a printer in Print Center to see its unprintable border, or set a safe margin in the View menu.",
        "Elige una impresora en el Centro de impresión para ver su borde no imprimible, o define un margen de seguridad en el menú Ver.",
    ),
    (
        "Select printer media in Print Center to see its unprintable border, or set a safe margin in the View menu.",
        "Selecciona el tamaño de papel en el Centro de impresión para ver su borde no imprimible, o define un margen de seguridad en el menú Ver.",
    ),
    (
        "Choose media that matches this label's size to see the printer's unprintable border, or set a safe margin in the View menu.",
        "Elige un papel que coincida con el tamaño de esta etiqueta para ver el borde no imprimible de la impresora, o define un margen de seguridad en el menú Ver.",
    ),
    (
        "The printer's driver reports no unprintable border for this media. Set a safe margin in the View menu to keep content away from the edge.",
        "El controlador de la impresora no indica ningún borde no imprimible para este papel. Define un margen de seguridad en el menú Ver para mantener el contenido lejos del borde.",
    ),
    (
        "Keeps designs this far from the label's edge. Use it when your printer cuts off the edges but its driver reports no unprintable border. It is remembered for each printer.",
        "Mantiene los diseños a esta distancia del borde de la etiqueta. Úsalo cuando tu impresora recorta los bordes pero su controlador no indica ningún borde no imprimible. Se recuerda para cada impresora.",
    ),
    ("Logo in the center", "Logotipo en el centro"),
    (
        "A logo hides part of the code. ThermalPrint keeps it small enough, and uses the highest error correction, so phones can still scan it.",
        "Un logotipo oculta parte del código. ThermalPrint lo mantiene lo bastante pequeño y usa la máxima corrección de errores, para que los teléfonos puedan escanearlo.",
    ),
    ("Add logo…", "Añadir logotipo…"),
    ("Change logo…", "Cambiar logotipo…"),
    ("Remove logo", "Quitar logotipo"),
    ("Choose a saved logo", "Elegir un logotipo guardado"),
    ("Logo size", "Tamaño del logotipo"),
    (
        "The share of the code's width that is cleared for the logo",
        "La parte del ancho del código que se despeja para el logotipo",
    ),
    (
        "Largest size that scans reliably:",
        "Tamaño máximo que se escanea con fiabilidad:",
    ),
    (
        "Error correction is set to High while a logo is used.",
        "La corrección de errores se fija en Alta mientras se usa un logotipo.",
    ),
    (
        "The code is small for a logo. Make it at least this wide:",
        "El código es pequeño para un logotipo. Hazlo al menos así de ancho:",
    ),
    ("No printer selected", "Ninguna impresora seleccionada"),
    (
        "No recent documents yet. Labels you open will appear here.",
        "Aún no hay documentos recientes. Las etiquetas que abras aparecerán aquí.",
    ),
    ("Reviewed", "Revisada"),
    (
        "Print this label (Ctrl+P)",
        "Imprimir esta etiqueta (Ctrl+P)",
    ),
    ("Add a label first", "Añade primero una etiqueta"),
    (
        "The preview is still being prepared",
        "La vista previa aún se está preparando",
    ),
    (
        "The label would be clipped. Adjust the layout",
        "La etiqueta quedaría recortada. Ajusta el diseño",
    ),
    (
        "Tick Reviewed once you have checked the crop",
        "Marca Revisada cuando hayas comprobado el recorte",
    ),
    (
        "Wait for the current operation to finish",
        "Espera a que termine la operación actual",
    ),
    ("Undo", "Deshacer"),
    ("Dismiss", "Descartar"),
    ("Watching", "Vigilando"),
    ("Hide this", "Ocultar esto"),
    ("Turn watching back on", "Reactivar la vigilancia"),
    ("Recent messages", "Mensajes recientes"),
    ("Show recent messages", "Mostrar mensajes recientes"),
    ("Add from Processed…", "Añadir desde Processed…"),
    (
        "Keep going if a label fails",
        "Continuar si una etiqueta falla",
    ),
    ("Open Import & Convert", "Abrir Importar y convertir"),
    ("Messages", "Mensajes"),
    ("Will print", "Se imprimirá"),
    (
        "Labels added from now on count as reviewed, so Print all does not wait for you to check each crop. Turn this on only for labels whose layout you trust.",
        "Las etiquetas añadidas a partir de ahora cuentan como revisadas, así que Imprimir todo no espera a que compruebes cada recorte. Actívalo solo para etiquetas cuyo diseño conoces.",
    ),
    (
        "Choose labels from the Processed folder to print them again",
        "Elige etiquetas de la carpeta Processed para imprimirlas de nuevo",
    ),
    (
        "Print all skips a label that fails and continues with the rest. Failed labels stay in the queue.",
        "Imprimir todo omite una etiqueta que falla y continúa con el resto. Las etiquetas fallidas permanecen en la cola.",
    ),
    ("Nothing yet", "Aún nada"),
    ("Current printer", "Impresora actual"),
    (
        "Printer for watched labels",
        "Impresora para etiquetas vigiladas",
    ),
    (
        "Skip review for automatic crops",
        "Omitir la revisión de recortes automáticos",
    ),
    ("Get ready to print", "Prepárate para imprimir"),
    (
        "Choose your label printer in Import & Convert",
        "Elige tu impresora de etiquetas en Importar y convertir",
    ),
    (
        "Add your label printer in your system settings, then reopen the app",
        "Añade tu impresora de etiquetas en los ajustes del sistema y vuelve a abrir la aplicación",
    ),
    (
        "Choose the label size your printer has loaded",
        "Elige el tamaño de etiqueta que tiene cargado tu impresora",
    ),
    (
        "Optional: watch a folder such as Downloads so new labels appear by themselves",
        "Opcional: vigila una carpeta como Descargas para que las etiquetas nuevas aparezcan solas",
    ),
    ("Restore your last queue?", "¿Restaurar la última cola?"),
    (
        "Move printed labels to a Processed folder",
        "Mover las etiquetas impresas a una carpeta Processed",
    ),
    (
        "After printing, labels from the watch folder are removed from the queue and moved into a Processed folder inside it. Labels added from elsewhere are not moved.",
        "Tras imprimir, las etiquetas de la carpeta vigilada se quitan de la cola y se mueven a una carpeta Processed dentro de ella. Las etiquetas añadidas desde otro lugar no se mueven.",
    ),
    ("About ThermalPrint", "Acerca de ThermalPrint"),
    ("Add labels…", "Añadir etiquetas…"),
    (
        "Advanced driver resolution",
        "Resolución avanzada del controlador",
    ),
    ("Copies", "Copias"),
    ("Height", "Alto"),
    ("Width", "Ancho"),
    ("W", "An"),
    ("H", "Al"),
    (
        "Import PDF or image labels (Ctrl+O)",
        "Importar etiquetas PDF o de imagen (Ctrl+O)",
    ),
    ("Printing service: CUPS", "Servicio de impresión: CUPS"),
    (
        "Refresh available CUPS printers",
        "Actualizar las impresoras CUPS disponibles",
    ),
    ("Rendering DPI", "DPI de renderizado"),
    ("Save", "Guardar"),
    ("Serial variable", "Variable de serie"),
    (
        "Swap label width and height",
        "Intercambiar ancho y alto de la etiqueta",
    ),
    ("Swap portrait / landscape", "Cambiar vertical / horizontal"),
    ("Templates", "Plantillas"),
    ("Zoom in", "Acercar"),
    ("Zoom out", "Alejar"),
    (
        "A successful submission means CUPS accepted the job; check physical output and barcode readability.",
        "Enviar correctamente significa que CUPS aceptó el trabajo; comprueba la impresión física y que el código de barras se lea bien.",
    ),
    ("Actual size", "Tamaño real"),
    (
        "Add new PDF and image labels automatically when they appear in a folder, such as Downloads. Files already there when you turn this on are ignored; labels saved while ThermalPrint is closed are added when it opens.",
        "Añade automáticamente las nuevas etiquetas PDF e imagen cuando aparecen en una carpeta, como Descargas. Los archivos que ya estaban al activarlo se ignoran; las etiquetas guardadas con ThermalPrint cerrado se añaden al abrirlo.",
    ),
    ("Alignment", "Alineación"),
    ("Back", "Atrás"),
    ("Backward", "Hacia atrás"),
    (
        "Barcode or QR code content",
        "Contenido del código de barras o QR",
    ),
    (
        "Black-and-white threshold (advanced)",
        "Umbral de blanco y negro (avanzado)",
    ),
    ("Bold", "Negrita"),
    ("Cancel", "Cancelar"),
    ("Center", "Centro"),
    (
        "Check CUPS job status",
        "Comprobar el estado de los trabajos CUPS",
    ),
    (
        "Check jobs submitted to the selected CUPS printer",
        "Comprueba los trabajos enviados a la impresora CUPS seleccionada",
    ),
    (
        "Check jobs submitted to the selected printer",
        "Comprueba los trabajos enviados a la impresora seleccionada",
    ),
    (
        "Check the original crop, then confirm it in the left panel.",
        "Revisa el recorte original y confírmalo en el panel izquierdo.",
    ),
    ("Choose CSV file…", "Elegir archivo CSV…"),
    (
        "Choose a printer in Print Center first.",
        "Elige primero una impresora en el Centro de impresión.",
    ),
    (
        "Choose a printer in Print Center to print this design. Export is available without a printer.",
        "Elige una impresora en el Centro de impresión para imprimir este diseño. Puedes exportar sin impresora.",
    ),
    ("Choose folder…", "Elegir carpeta…"),
    ("Clear", "Quitar"),
    ("Clear history", "Borrar historial"),
    ("Close", "Cerrar"),
    (
        "Copy version information",
        "Copiar información de la versión",
    ),
    ("Credits and license", "Créditos y licencia"),
    ("Custom scale", "Escala personalizada"),
    (
        "Default processing preserves tones. Thresholding can damage fine barcode detail.",
        "El procesado predeterminado conserva los tonos. El umbral puede dañar los detalles finos del código de barras.",
    ),
    ("Delete", "Eliminar"),
    (
        "Delete all selected objects",
        "Eliminar todos los objetos seleccionados",
    ),
    ("Delete selected", "Eliminar selección"),
    (
        "Delete selected objects",
        "Eliminar los objetos seleccionados",
    ),
    (
        "Design a custom label or prepare a downloaded shipping label.",
        "Diseña una etiqueta propia o prepara una etiqueta de envío descargada.",
    ),
    (
        "Design and prepare thermal labels for Linux.",
        "Diseña y prepara etiquetas térmicas para Linux.",
    ),
    ("Discard", "Descartar"),
    (
        "Drag to draw. Drag inside to move; drag a corner to resize. Crop coordinates refer to the original page.",
        "Arrastra para dibujar. Arrastra dentro para mover; arrastra una esquina para cambiar el tamaño. Las coordenadas del recorte se refieren a la página original.",
    ),
    (
        "Drag to move · square to resize · circle to rotate · Shift: 15°",
        "Arrastra para mover · cuadrado para redimensionar · círculo para girar · Mayús: 15°",
    ),
    (
        "Drag to rotate freely. Hold Shift to snap to 15°.",
        "Arrastra para girar libremente. Mantén Mayús para ajustar a 15°.",
    ),
    (
        "Driver default resolution",
        "Resolución predeterminada del controlador",
    ),
    (
        "Drop PDFs or images to build a print queue",
        "Suelta PDF o imágenes para crear una cola de impresión",
    ),
    (
        "Drop a downloaded label anywhere.",
        "Suelta una etiqueta descargada en cualquier lugar.",
    ),
    ("Duplicate", "Duplicar"),
    (
        "Duplicate selected objects (Ctrl+D)",
        "Duplicar los objetos seleccionados (Ctrl+D)",
    ),
    (
        "Embedded image. Resize with W/H or the canvas handle.",
        "Imagen incrustada. Cambia el tamaño con An/Al o con el tirador del lienzo.",
    ),
    (
        "Enter your real product code or reference",
        "Introduce tu código de producto o referencia real",
    ),
    (
        "Every row is checked first; nothing prints if a row has a problem.",
        "Primero se revisan todas las filas; no se imprime nada si alguna tiene un problema.",
    ),
    ("Explore ThermalPrint", "Explorar ThermalPrint"),
    (
        "Export a CSV from your spreadsheet. Column names are not case-sensitive; ${serial} counts up from the design's serial number.",
        "Exporta un CSV desde tu hoja de cálculo. Los nombres de columna no distinguen mayúsculas; ${serial} cuenta desde el número de serie del diseño.",
    ),
    ("Export", "Exportar"),
    (
        "Export the prepared label as PDF",
        "Exporta la etiqueta preparada como PDF",
    ),
    (
        "Export the prepared label as PNG",
        "Exporta la etiqueta preparada como PNG",
    ),
    ("File", "Archivo"),
    ("Filled", "Relleno"),
    (
        "Fit label to canvas and reset panning",
        "Ajustar la etiqueta al lienzo y restablecer el desplazamiento",
    ),
    (
        "Fit label to the preview area",
        "Ajustar la etiqueta al área de vista previa",
    ),
    ("Fit within paper", "Ajustar al papel"),
    ("Focus canvas", "Enfocar el lienzo"),
    ("Font size in points", "Tamaño de fuente en puntos"),
    ("Forward", "Hacia delante"),
    (
        "From any marketplace or carrier. No account needed.",
        "De cualquier tienda en línea o transportista. No se necesita cuenta.",
    ),
    (
        "Hide navigation and properties to expand the canvas",
        "Oculta la navegación y las propiedades para ampliar el lienzo",
    ),
    ("Horizontal", "Horizontal"),
    (
        "Horizontal print offset in millimeters",
        "Desplazamiento horizontal de impresión en milímetros",
    ),
    (
        "I checked the crop and barcode edges",
        "He revisado el recorte y los bordes del código de barras",
    ),
    (
        "If the loaded paper matches the printer media, match the output below. Otherwise select media that matches the label size you want.",
        "Si el papel cargado coincide con el papel de la impresora, ajusta la salida de abajo. Si no, elige un papel que coincida con el tamaño de etiqueta que quieres.",
    ),
    (
        "If the loaded paper matches the printer media, use the button below. To keep the current label size, choose matching media in Print Center instead.",
        "Si el papel cargado coincide con el papel de la impresora, usa el botón de abajo. Para mantener el tamaño actual de la etiqueta, elige en su lugar un papel equivalente en el Centro de impresión.",
    ),
    ("Job status", "Estado del trabajo"),
    ("Label", "Etiqueta"),
    (
        "Label height in millimeters",
        "Alto de la etiqueta en milímetros",
    ),
    ("Label name", "Nombre de la etiqueta"),
    (
        "Label setup (no selection)",
        "Configuración de la etiqueta (sin selección)",
    ),
    ("Label stock", "Papel de etiquetas"),
    (
        "Label width in millimeters",
        "Ancho de la etiqueta en milímetros",
    ),
    ("Labels, ready to go.", "Etiquetas listas para usar."),
    ("Left", "Izquierda"),
    ("MIT License", "Licencia MIT"),
    ("Main menu", "Menú principal"),
    (
        "Match label to printer media",
        "Ajustar la etiqueta al papel de la impresora",
    ),
    (
        "Match output to printer media",
        "Ajustar la salida al papel de la impresora",
    ),
    ("More from the author", "Más del autor"),
    (
        "Move label earlier in the print queue",
        "Mover la etiqueta antes en la cola de impresión",
    ),
    (
        "Move label later in the print queue",
        "Mover la etiqueta después en la cola de impresión",
    ),
    (
        "Move object one layer backward",
        "Mover el objeto una capa hacia atrás",
    ),
    (
        "Move object one layer forward",
        "Mover el objeto una capa hacia delante",
    ),
    ("New label", "Nueva etiqueta"),
    ("No folder chosen", "Ninguna carpeta elegida"),
    (
        "No printer selected. Export is available without a printer.",
        "No hay impresora seleccionada. Puedes exportar sin impresora.",
    ),
    ("Object", "Objeto"),
    (
        "Object height in millimeters",
        "Alto del objeto en milímetros",
    ),
    (
        "Object horizontal position in millimeters",
        "Posición horizontal del objeto en milímetros",
    ),
    ("Object text", "Texto del objeto"),
    (
        "Object vertical position in millimeters",
        "Posición vertical del objeto en milímetros",
    ),
    (
        "Object width in millimeters",
        "Ancho del objeto en milímetros",
    ),
    ("Open", "Abrir"),
    (
        "Open a saved label design",
        "Abre un diseño de etiqueta guardado",
    ),
    ("Open import queue", "Abrir la cola de importación"),
    (
        "Open imported labels for review",
        "Abre las etiquetas importadas para revisarlas",
    ),
    (
        "Open this label for review",
        "Abre esta etiqueta para revisarla",
    ),
    ("Original / crop", "Original / recorte"),
    (
        "Output label proportions",
        "Proporciones de la etiqueta de salida",
    ),
    ("Output label size", "Tamaño de la etiqueta de salida"),
    ("Output settings", "Ajustes de salida"),
    ("Page", "Página"),
    ("Prepare your label", "Prepara tu etiqueta"),
    ("Preview row", "Previsualizar fila"),
    ("Preview row number", "Número de fila de la vista previa"),
    (
        "Preview the final label as it will print",
        "Previsualiza la etiqueta final tal como se imprimirá",
    ),
    ("Print", "Imprimir"),
    ("Print Center", "Centro de impresión"),
    ("Print alignment test", "Imprimir prueba de alineación"),
    ("Print label   Ctrl+P", "Imprimir etiqueta   Ctrl+P"),
    (
        "Print label using the selected printer",
        "Imprime la etiqueta con la impresora seleccionada",
    ),
    (
        "Print one label per row of a spreadsheet, filling ${column} placeholders",
        "Imprime una etiqueta por fila de una hoja de cálculo, rellenando los marcadores ${column}",
    ),
    ("Print using raw ZPL", "Imprimir con ZPL sin procesar"),
    ("Printer & media", "Impresora y papel"),
    ("Printer queue", "Cola de la impresora"),
    ("Printer resolution", "Resolución de la impresora"),
    ("Printer status", "Estado de la impresora"),
    ("Printing terms", "Términos de impresión"),
    (
        "Prints a ruler label with a 1 mm border so you can measure the offset",
        "Imprime una etiqueta con regla y un borde de 1 mm para medir el desplazamiento",
    ),
    ("Properties", "Propiedades"),
    ("Properties panel", "Panel de propiedades"),
    (
        "Put ${column} placeholders in text, barcode, and QR content. Each row of your spreadsheet becomes one label.",
        "Pon marcadores ${column} en el texto, el código de barras y el contenido QR. Cada fila de tu hoja de cálculo se convierte en una etiqueta.",
    ),
    ("QR code content", "Contenido del código QR"),
    ("QR content", "Contenido QR"),
    ("Queue & diagnostics", "Cola y diagnóstico"),
    ("Quit", "Salir"),
    (
        "Raw ZPL: verify printer compatibility and loaded label size.",
        "ZPL sin procesar: verifica la compatibilidad de la impresora y el tamaño de etiqueta cargado.",
    ),
    ("Recent documents", "Documentos recientes"),
    ("Recent prints", "Impresiones recientes"),
    ("Redo (Ctrl+Y)", "Rehacer (Ctrl+Y)"),
    ("Refresh printers", "Actualizar impresoras"),
    (
        "Remove this label from the import queue",
        "Quita esta etiqueta de la cola de importación",
    ),
    (
        "Requires a ZPL-compatible printer and raw CUPS queue.",
        "Requiere una impresora compatible con ZPL y una cola CUPS sin procesar.",
    ),
    ("Reset offset", "Restablecer desplazamiento"),
    (
        "Reset the crop to use the entire original page",
        "Restablece el recorte para usar toda la página original",
    ),
    ("Reset to entire page", "Restablecer a la página completa"),
    ("Right", "Derecha"),
    ("Rotate", "Girar"),
    ("Rotation", "Rotación"),
    ("Save as…", "Guardar como…"),
    ("Save changes?", "¿Guardar los cambios?"),
    ("Save design (Ctrl+S)", "Guardar diseño (Ctrl+S)"),
    (
        "Save your changes to keep them, or discard them to continue.",
        "Guarda los cambios para conservarlos o descártalos para continuar.",
    ),
    (
        "Select an object on the canvas.",
        "Selecciona un objeto en el lienzo.",
    ),
    ("Select label object", "Seleccionar objeto de la etiqueta"),
    (
        "Select printer media in Print Center before printing.",
        "Elige el papel de la impresora en el Centro de impresión antes de imprimir.",
    ),
    (
        "Set the canvas dimensions to the selected printer stock. Object sizes and positions are preserved; Ctrl+Z undoes this change.",
        "Ajusta las dimensiones del lienzo al papel de impresora seleccionado. Se conservan los tamaños y posiciones de los objetos; Ctrl+Z deshace el cambio.",
    ),
    (
        "Set up your printer and check your next print run.",
        "Configura tu impresora y revisa tu próxima tanda de impresión.",
    ),
    (
        "Settings above are remembered for each printer. Use the offset if labels print slightly off-center.",
        "Los ajustes de arriba se recuerdan para cada impresora. Usa el desplazamiento si las etiquetas salen ligeramente descentradas.",
    ),
    (
        "Shape stroke width in millimeters",
        "Grosor del trazo de la forma en milímetros",
    ),
    (
        "Shift-click to select multiple objects. Use Ctrl+C/V, Ctrl+D, Delete, Ctrl+Z/Y.",
        "Mayús+clic para seleccionar varios objetos. Usa Ctrl+C/V, Ctrl+D, Supr, Ctrl+Z/Y.",
    ),
    ("Show grid", "Mostrar cuadrícula"),
    ("Show value below bars", "Mostrar el valor bajo las barras"),
    ("Snap to grid", "Ajustar a la cuadrícula"),
    (
        "Text · ${date} and ${serial} are dynamic",
        "Texto · ${date} y ${serial} son dinámicos",
    ),
    ("Thermal label", "Etiqueta térmica"),
    (
        "These dimensions control the output and stock preview. Match them to the printer media size above.",
        "Estas dimensiones controlan la salida y la vista previa del papel. Haz que coincidan con el tamaño de papel de la impresora de arriba.",
    ),
    (
        "This row cannot be previewed. Printing will report the problem.",
        "Esta fila no se puede previsualizar. Al imprimir se informará del problema.",
    ),
    ("Undo (Ctrl+Z)", "Deshacer (Ctrl+Z)"),
    (
        "Update the output width and height to match the selected printer stock",
        "Actualiza el ancho y alto de salida para que coincidan con el papel de impresora seleccionado",
    ),
    ("Vertical", "Vertical"),
    (
        "Vertical print offset in millimeters",
        "Desplazamiento vertical de impresión en milímetros",
    ),
    ("View", "Ver"),
    (
        "View the next label (Alt+Right)",
        "Ver la etiqueta siguiente (Alt+Right)",
    ),
    (
        "View the original document and adjust the crop",
        "Ver el documento original y ajustar el recorte",
    ),
    (
        "View the previous label (Alt+Left)",
        "Ver la etiqueta anterior (Alt+Left)",
    ),
    ("WORKSPACE", "ESPACIO DE TRABAJO"),
    ("Watch a folder", "Vigilar una carpeta"),
    ("Watch for new labels", "Vigilar etiquetas nuevas"),
    (
        "Your label has unsaved changes.",
        "Tu etiqueta tiene cambios sin guardar.",
    ),
    ("Your label workspace", "Tu espacio de etiquetas"),
    ("Home", "Inicio"),
    ("Label Designer", "Diseñador de etiquetas"),
    ("Import & Convert", "Importar y convertir"),
    (
        "Open your workspace and label templates",
        "Abre tu espacio de trabajo y las plantillas de etiquetas",
    ),
    (
        "Create and edit a custom label",
        "Crea y edita una etiqueta personalizada",
    ),
    (
        "Import, crop, and prepare existing labels",
        "Importa, recorta y prepara etiquetas existentes",
    ),
    (
        "Configure printer, stock, and queue settings",
        "Configura la impresora, el papel y la cola",
    ),
    ("Create a label", "Crear una etiqueta"),
    ("Build something of your own", "Construye algo propio"),
    (
        "Add text, shapes, barcodes and QR codes to a precisely sized canvas.",
        "Añade texto, formas, códigos de barras y códigos QR a un lienzo de tamaño exacto.",
    ),
    ("Create new label", "Crear etiqueta nueva"),
    ("Import & convert", "Importar y convertir"),
    (
        "Get existing labels ready to print",
        "Prepara etiquetas existentes para imprimir",
    ),
    (
        "Open PDFs or images, remove instructions, review crops, then print in order.",
        "Abre PDF o imágenes, quita las instrucciones, revisa los recortes e imprime en orden.",
    ),
    ("Open existing labels", "Abrir etiquetas existentes"),
    ("Project & source code", "Proyecto y código fuente"),
    (
        "Documentation, releases, and source",
        "Documentación, versiones y código",
    ),
    ("Report an issue", "Informar de un problema"),
    (
        "Share a bug or suggest an improvement",
        "Comparte un error o sugiere una mejora",
    ),
    ("Created by", "Creado por"),
    (
        "Free to use, modify, and share",
        "Libre para usar, modificar y compartir",
    ),
    ("Thermal labels", "Etiquetas térmicas"),
    ("Linux app", "App para Linux"),
    ("Open source", "Código abierto"),
    ("Version copied", "Versión copiada"),
    (
        "Power tools for your desktop",
        "Herramientas potentes para tu escritorio",
    ),
    ("Battery health for GNOME", "Salud de la batería para GNOME"),
    ("Version", "Versión"),
    ("Batch print from CSV", "Imprimir por lotes desde CSV"),
    ("Batch print from CSV…", "Imprimir por lotes desde CSV…"),
    ("Preparing", "Preparando"),
    (
        "Wait for the current operation to finish.",
        "Espera a que termine la operación actual.",
    ),
    ("Add a label", "Añade una etiqueta"),
    (
        "Import a PDF or image to start preparing a print.",
        "Importa un PDF o una imagen para empezar a preparar una impresión.",
    ),
    ("Adjust the layout", "Ajusta la disposición"),
    (
        "The label would be clipped. Use Fit within paper or change the output dimensions.",
        "La etiqueta se recortaría. Usa «Ajustar al papel» o cambia las dimensiones de salida.",
    ),
    ("Review the crop", "Revisa el recorte"),
    (
        "Check the original crop and barcode edges, then confirm the review in Extract.",
        "Revisa el recorte original y los bordes del código de barras, y confirma la revisión en Extraer.",
    ),
    ("Preparing preview", "Preparando la vista previa"),
    (
        "The label preview is being generated.",
        "Se está generando la vista previa de la etiqueta.",
    ),
    ("Choose a printer", "Elige una impresora"),
    (
        "Select an available printer before printing. You can still export a prepared label.",
        "Selecciona una impresora disponible antes de imprimir. Aún puedes exportar una etiqueta preparada.",
    ),
    ("Choose printer media", "Elige el papel de la impresora"),
    (
        "Select a supported media size that matches the loaded labels.",
        "Selecciona un tamaño de papel compatible que coincida con las etiquetas cargadas.",
    ),
    (
        "Label size differs from printer media",
        "El tamaño de la etiqueta difiere del papel de la impresora",
    ),
    ("Ready to submit", "Listo para enviar"),
    ("Printer configured", "Impresora configurada"),
    ("Verify the loaded stock", "Verifica el papel cargado"),
    (
        "The driver does not report dimensions for this media. Check that the loaded labels match the output size.",
        "El controlador no informa de las dimensiones de este papel. Comprueba que las etiquetas cargadas coincidan con el tamaño de salida.",
    ),
    (
        "Crop reviewed and output prepared. Check the loaded stock before printing.",
        "Recorte revisado y salida preparada. Comprueba el papel cargado antes de imprimir.",
    ),
    (
        "Media matches the output dimensions. Import or design a label to print.",
        "El papel coincide con las dimensiones de salida. Importa o diseña una etiqueta para imprimir.",
    ),
    ("Blank custom", "En blanco"),
    ("Shipping 4 × 6", "Envío 4 × 6"),
    ("Return address", "Remitente"),
    ("Product barcode", "Código de barras de producto"),
    ("Inventory", "Inventario"),
    ("QR code", "Código QR"),
    ("Asset tracking", "Seguimiento de activos"),
    ("Shelf", "Estantería"),
    ("Price tag", "Etiqueta de precio"),
    ("Name label", "Etiqueta de nombre"),
    ("Fragile", "Frágil"),
    ("Help", "Ayuda"),
    ("Help & Manual", "Ayuda y manual"),
    ("Tray icon restored", "Icono de la bandeja restaurado"),
    (
        "Open the user manual in your browser (F1)",
        "Abre el manual de usuario en tu navegador (F1)",
    ),
    ("Save as template…", "Guardar como plantilla…"),
    (
        "Keep this design in your Templates menu, named after the label",
        "Conserva este diseño en tu menú de plantillas, con el nombre de la etiqueta",
    ),
    ("Smart guides", "Guías inteligentes"),
    (
        "Snap to other objects' edges and centers while dragging",
        "Se ajusta a los bordes y centros de otros objetos al arrastrar",
    ),
    ("Show printable area", "Mostrar área imprimible"),
    (
        "Shade the border your printer cannot print on",
        "Sombrea el borde donde tu impresora no puede imprimir",
    ),
    (
        "These objects reach into the printer's unprintable border and may be cut off:",
        "Estos objetos llegan al borde no imprimible de la impresora y pueden cortarse:",
    ),
    ("Your templates", "Tus plantillas"),
    ("Remove this template", "Quitar esta plantilla"),
    ("Grid size", "Tamaño de cuadrícula"),
    (
        "Grid size in millimeters",
        "Tamaño de la cuadrícula en milímetros",
    ),
    ("Align", "Alinear"),
    ("Distribute evenly", "Distribuir uniformemente"),
    ("Center on label", "Centrar en la etiqueta"),
    ("Lock all", "Bloquear todo"),
    ("Unlock all", "Desbloquear todo"),
    ("Rotate 90°", "Girar 90°"),
    (
        "Turn the object a quarter turn clockwise",
        "Gira el objeto un cuarto de vuelta a la derecha",
    ),
    ("Flip horizontally", "Voltear horizontalmente"),
    ("Flip vertically", "Voltear verticalmente"),
    ("Lock position and size", "Bloquear posición y tamaño"),
    ("Italic", "Cursiva"),
    ("Inverse", "Invertido"),
    (
        "White letters on a black box",
        "Letras blancas sobre un recuadro negro",
    ),
    ("Wrap long lines", "Ajustar líneas largas"),
    ("Shrink to fit the box", "Reducir para ajustar al recuadro"),
    ("Line spacing", "Interlineado"),
    ("Line spacing multiplier", "Multiplicador del interlineado"),
    ("Text size", "Tamaño del texto"),
    (
        "Readable text size in points",
        "Tamaño del texto legible en puntos",
    ),
    ("Text above the bars", "Texto sobre las barras"),
    (
        "QR error correction level",
        "Nivel de corrección de errores del QR",
    ),
    ("Contrast", "Contraste"),
    (
        "Image contrast in percent",
        "Contraste de la imagen en porcentaje",
    ),
    ("Cut-off", "Umbral"),
    (
        "Pixels darker than this print black",
        "Los píxeles más oscuros que esto se imprimen en negro",
    ),
    ("Invert black and white", "Invertir blanco y negro"),
    ("Reset image adjustments", "Restablecer ajustes de imagen"),
    (
        "Spacing of the guide grid. Snapping moves in steps of a fifth of it.",
        "Separación de la cuadrícula guía. El ajuste se mueve en pasos de un quinto.",
    ),
    (
        "A locked object cannot be moved, resized, rotated, or deleted from the canvas",
        "Un objeto bloqueado no se puede mover, redimensionar, girar ni borrar desde el lienzo",
    ),
    (
        "Break lines at spaces so they fit the box width",
        "Divide las líneas en los espacios para que quepan en el ancho del recuadro",
    ),
    (
        "Make the text smaller until all of it fits inside the box",
        "Reduce el texto hasta que quepa entero en el recuadro",
    ),
    (
        "More error correction survives more damage but makes a denser code",
        "Más corrección de errores resiste más daño pero crea un código más denso",
    ),
    (
        "How the image is reduced to black and white",
        "Cómo se reduce la imagen a blanco y negro",
    ),
    ("Align left", "Alinear a la izquierda"),
    ("Align center", "Alinear al centro"),
    ("Align right", "Alinear a la derecha"),
    ("Align top", "Alinear arriba"),
    ("Align middle", "Alinear en el medio"),
    ("Align bottom", "Alinear abajo"),
    ("Distribute horizontally", "Distribuir horizontalmente"),
    ("Distribute vertically", "Distribuir verticalmente"),
    (
        "Center horizontally on label",
        "Centrar horizontalmente en la etiqueta",
    ),
    (
        "Center vertically on label",
        "Centrar verticalmente en la etiqueta",
    ),
    (
        "Lock all selected objects",
        "Bloquea todos los objetos seleccionados",
    ),
    (
        "Unlock all selected objects",
        "Desbloquea todos los objetos seleccionados",
    ),
    (
        "Return the image mode, contrast, and cut-off to their defaults",
        "Devuelve el modo de imagen, el contraste y el umbral a sus valores predeterminados",
    ),
    ("Website", "Sitio web"),
    ("Wi-Fi", "Wi-Fi"),
    ("Contact card", "Tarjeta de contacto"),
    ("Email", "Correo electrónico"),
    ("Phone number", "Número de teléfono"),
    ("Build the content", "Crear el contenido"),
    ("Kind of QR content", "Tipo de contenido QR"),
    ("Web address", "Dirección web"),
    ("Network name", "Nombre de la red"),
    ("Password", "Contraseña"),
    ("WPA / WPA2", "WPA / WPA2"),
    ("WEP", "WEP"),
    ("No password", "Sin contraseña"),
    ("Hidden network", "Red oculta"),
    ("Name", "Nombre"),
    ("Organization", "Organización"),
    ("Phone", "Teléfono"),
    ("Email address", "Dirección de correo"),
    ("Address", "Dirección"),
    ("Subject", "Asunto"),
    ("Message", "Mensaje"),
    ("Use this content", "Usar este contenido"),
    (
        "Replace the QR content with what you entered",
        "Reemplaza el contenido del QR con lo que escribiste",
    ),
    (
        "Unsaved changes from your last session were found:",
        "Se encontraron cambios sin guardar de tu última sesión:",
    ),
    ("Restore", "Restaurar"),
    (
        "Reopen the design from your last session as an unsaved copy",
        "Reabre el diseño de tu última sesión como copia sin guardar",
    ),
    ("Delete the recovered copy", "Elimina la copia recuperada"),
    ("Template gallery", "Galería de plantillas"),
    ("Search", "Buscar"),
    ("Search templates", "Buscar plantillas"),
    (
        "Nothing saved yet. Use File → Save as template… to add one.",
        "Aún no hay nada guardado. Usa Archivo → Guardar como plantilla… para añadir una.",
    ),
    ("Print on a sheet", "Imprimir en una hoja"),
    ("Paper", "Papel"),
    ("Resolution", "Resolución"),
    ("Columns", "Columnas"),
    ("Rows", "Filas"),
    ("Page margin", "Margen de página"),
    (
        "Page margin in millimeters",
        "Margen de página en milímetros",
    ),
    ("Gap between labels", "Espacio entre etiquetas"),
    (
        "Gap between labels in millimeters",
        "Espacio entre etiquetas en milímetros",
    ),
    ("How many labels", "Cuántas etiquetas"),
    ("Number of labels", "Número de etiquetas"),
    ("Fit as many as possible", "Ajustar tantas como quepan"),
    (
        "Choose the most columns and rows of this label size that fit the page",
        "Elige el máximo de columnas y filas de este tamaño de etiqueta que caben en la página",
    ),
    (
        "The selected printer does not list this paper. You can still save a PDF.",
        "La impresora seleccionada no incluye este papel. Aun así puedes guardar un PDF.",
    ),
    ("Save as PDF", "Guardar como PDF"),
    (
        "Save the sheet as a PDF file",
        "Guarda la hoja como archivo PDF",
    ),
    ("Print sheet", "Imprimir hoja"),
    (
        "Send the sheet to the selected printer",
        "Envía la hoja a la impresora seleccionada",
    ),
    ("labels per page", "etiquetas por página"),
    ("page", "página"),
    ("pages", "páginas"),
    ("Print a number range", "Imprimir un rango de números"),
    (
        "Put ${serial} in text, barcode, or QR content. Use ${serial:4} for numbers padded with zeros, like 0007. Add a prefix by typing it before the number, for example INV-${serial:4}.",
        "Pon ${serial} en el texto, código de barras o contenido QR. Usa ${serial:4} para números con ceros a la izquierda, como 0007. Añade un prefijo escribiéndolo antes del número, por ejemplo INV-${serial:4}.",
    ),
    ("First number", "Primer número"),
    ("Step", "Incremento"),
    ("Step between numbers", "Incremento entre números"),
    ("Serials", "Series"),
    (
        "This design has no ${serial} yet, so every label would be the same.",
        "Este diseño aún no tiene ${serial}, así que todas las etiquetas serían iguales.",
    ),
    ("Print labels:", "Imprimir etiquetas:"),
    (
        "Every label is checked first; nothing prints if one has a problem.",
        "Primero se revisa cada etiqueta; no se imprime nada si alguna tiene un problema.",
    ),
    ("Template gallery…", "Galería de plantillas…"),
    (
        "Browse the built-in templates and your own with previews",
        "Explora las plantillas incluidas y las tuyas con vistas previas",
    ),
    ("Print on a sheet…", "Imprimir en una hoja…"),
    ("Print a number range…", "Imprimir un rango de números…"),
    (
        "Print labels numbered by ${serial} with a start, step, and count",
        "Imprime etiquetas numeradas con ${serial} según inicio, incremento y cantidad",
    ),
    ("Address book", "Libreta de direcciones"),
    (
        "Nothing saved yet. Select a text object and choose Save to address book.",
        "Aún no hay nada guardado. Selecciona un texto y elige Guardar en la libreta de direcciones.",
    ),
    ("Remove this address", "Quitar esta dirección"),
    ("Symbols", "Símbolos"),
    ("Your logos", "Tus logotipos"),
    ("Remove this logo", "Quitar este logotipo"),
    ("Symbols and logos", "Símbolos y logotipos"),
    (
        "Auto-length (continuous roll)",
        "Longitud automática (rollo continuo)",
    ),
    (
        "Print only as long as the content needs, plus a bottom margin. Use it with continuous label rolls.",
        "Imprime solo el largo que necesita el contenido, más un margen inferior. Úsalo con rollos de etiquetas continuos.",
    ),
    ("Bottom margin", "Margen inferior"),
    (
        "Bottom margin in millimeters",
        "Margen inferior en milímetros",
    ),
    ("Printed length", "Longitud impresa"),
    (
        "Save to address book",
        "Guardar en la libreta de direcciones",
    ),
    (
        "Keep this text, named after its first line, in your address book",
        "Guarda este texto en tu libreta de direcciones, con el nombre de su primera línea",
    ),
    ("Font", "Fuente"),
    (
        "This font is not installed on this computer, so the built-in font is used.",
        "Esta fuente no está instalada en este equipo, así que se usa la fuente incluida.",
    ),
    (
        "Add a font from a .ttf or .otf file",
        "Añade una fuente desde un archivo .ttf u .otf",
    ),
    ("Save as logo", "Guardar como logotipo"),
    (
        "Keep this image under Symbols and logos so you can add it to any label",
        "Guarda esta imagen en Símbolos y logotipos para añadirla a cualquier etiqueta",
    ),
    ("Prints as", "Se imprime como"),
    ("SSCC helper", "Ayuda para SSCC"),
    (
        "An SSCC numbers one pallet or parcel: extension digit, your GS1 company prefix, and a serial reference. The check digit is added for you.",
        "Un SSCC numera un palé o paquete: dígito de extensión, tu prefijo de empresa GS1 y una referencia de serie. El dígito de control se añade por ti.",
    ),
    ("Extension digit", "Dígito de extensión"),
    ("Company prefix", "Prefijo de empresa"),
    ("Serial reference", "Referencia de serie"),
    ("Use this SSCC", "Usar este SSCC"),
    (
        "Replace the barcode content with (00) and the SSCC",
        "Reemplaza el contenido del código de barras con (00) y el SSCC",
    ),
    (
        "The logistics barcode for pallets and cartons. Type GS1 data such as (01)09506000134352(10)LOT7.",
        "El código de barras logístico para palés y cajas. Escribe datos GS1 como (01)09506000134352(10)LOT7.",
    ),
    (
        "A small square symbol for GS1 data such as (01)09506000134352(10)LOT7. Put a batch or serial last.",
        "Un símbolo cuadrado pequeño para datos GS1 como (01)09506000134352(10)LOT7. Pon el lote o la serie al final.",
    ),
    ("Rotate 90° when printing", "Girar 90° al imprimir"),
    (
        "For wide content: design the label turned on its side so the text reads straight, and it is rotated 90° when it prints, to fit the label stock as loaded.",
        "Para contenido ancho: diseña la etiqueta girada de lado para que el texto se lea recto; al imprimir se gira 90° para que quepa en las etiquetas tal como están cargadas.",
    ),
    (
        "Rotated 90° counterclockwise to print on",
        "Se gira 90° en sentido antihorario para imprimir en",
    ),
    (
        "Rotated 90° clockwise to print on",
        "Se gira 90° en sentido horario para imprimir en",
    ),
    ("Rotate the other way", "Girar al otro lado"),
    (
        "Which way the design faces on the printed label. Check “As printed” beside the canvas.",
        "Hacia dónde mira el diseño en la etiqueta impresa. Revisa «Impresa» junto al lienzo.",
    ),
    (
        "Not available with rotated printing.",
        "No disponible con la impresión girada.",
    ),
    ("Rotated printing.", "Impresión girada."),
    (
        "You are designing the label on its side. When it prints, it is rotated 90° counterclockwise to fit",
        "Estás diseñando la etiqueta de lado. Al imprimir, se gira 90° en sentido antihorario para caber en",
    ),
    (
        "You are designing the label on its side. When it prints, it is rotated 90° clockwise to fit",
        "Estás diseñando la etiqueta de lado. Al imprimir, se gira 90° en sentido horario para caber en",
    ),
    (
        "See “As printed” in the corner.",
        "Mira «Impresa» en la esquina.",
    ),
    ("As printed", "Impresa"),
    (
        "Rotated printing: Print rotates it 90° to fit",
        "Impresión girada: Imprimir la gira 90° para caber en",
    ),
    ("Default (DejaVu Sans)", "Predeterminada (DejaVu Sans)"),
    (
        "Built in · prints the same everywhere",
        "Incluidas · se imprimen igual en todas partes",
    ),
    ("Your fonts", "Tus fuentes"),
    ("On this computer", "En este equipo"),
    ("Search fonts", "Buscar fuentes"),
    ("No fonts match.", "Ninguna fuente coincide."),
    (
        "This font has no bold style. Choose a bold font instead.",
        "Esta fuente no tiene estilo negrita. Elige una fuente negrita.",
    ),
    (
        "Another computer needs this font installed to print the label the same way.",
        "Otro equipo necesita esta fuente instalada para imprimir la etiqueta igual.",
    ),
    (
        "Condensed (Noto Sans Condensed)",
        "Estrecha (Noto Sans Condensed)",
    ),
    (
        "Monospace (DejaVu Sans Mono)",
        "Monoespaciada (DejaVu Sans Mono)",
    ),
    ("Serif (DejaVu Serif)", "Con serifa (DejaVu Serif)"),
    (
        "Headline (Noto Sans Display ExtraCondensed Black)",
        "Titular (Noto Sans Display ExtraCondensed Black)",
    ),
    ("Rounded (Varela Round)", "Redondeada (Varela Round)"),
    ("Choose a font", "Elegir una fuente"),
    // Label footer
    ("Label footer", "Pie de etiqueta"),
    (
        "The footer QR code was left out because there is not enough blank space at the bottom of this label for it. The message still prints.",
        "Se omitió el código QR del pie porque no hay suficiente espacio en blanco en la parte inferior de esta etiqueta. El mensaje se imprime igualmente.",
    ),
    ("Print a footer message", "Imprimir un mensaje al pie"),
    ("Footer message", "Mensaje al pie"),
    ("QR code link or text", "Enlace o texto del código QR"),
    ("Footer QR code content", "Contenido del código QR del pie"),
    (
        "Enter a message to print the footer. The QR code prints only with a message.",
        "Escribe un mensaje para imprimir el pie. El código QR solo se imprime junto con un mensaje.",
    ),
    (
        "Scan a test print with your phone before printing a batch.",
        "Escanea una impresión de prueba con tu teléfono antes de imprimir un lote.",
    ),
    ("Reset footer", "Restablecer pie"),
    (
        "The QR text is too long to encode. Shorten it.",
        "El texto del QR es demasiado largo para codificarlo. Acórtalo.",
    ),
    (
        "The footer QR code was left out because it would be large on this short label. The message still prints.",
        "Se omitió el código QR del pie porque sería grande en esta etiqueta corta. El mensaje se imprime igualmente.",
    ),
    // 0.15.0: designer footer, overlap warning, symbols
    (
        "Add a QR code before the message",
        "Añadir un código QR antes del mensaje",
    ),
    (
        "Turn the footer off and restore the default message, QR code link, and alignment",
        "Desactivar el pie y restaurar el mensaje, el enlace del código QR y la alineación predeterminados",
    ),
    (
        "The footer was left out: part of the design is in the blank space it needs at the bottom of the label. Move objects up, choose another alignment, or turn the footer off. The label size is unchanged.",
        "Se omitió el pie: parte del diseño ocupa el espacio en blanco que necesita en la parte inferior de la etiqueta. Sube los objetos, elige otra alineación o desactiva el pie. El tamaño de la etiqueta no cambia.",
    ),
    (
        "The footer was left out: the label is too small for it inside the printable area.",
        "Se omitió el pie: la etiqueta es demasiado pequeña para él dentro del área imprimible.",
    ),
    (
        "Adds a small message, and if you like a QR code, along the bottom of labels you design here when you print them. Imported shipping labels never get a footer.",
        "Añade un mensaje pequeño y, si quieres, un código QR en la parte inferior de las etiquetas que diseñas aquí al imprimirlas. Las etiquetas de envío importadas nunca llevan pie.",
    ),
    (
        "Choose where the whole footer sits along the bottom edge. Pick a side where your design leaves blank space.",
        "Elige dónde va todo el pie a lo largo del borde inferior. Escoge un lado donde tu diseño deje espacio en blanco.",
    ),
    (
        "The label size and printable area never change. If that spot is not blank or is outside the printable area, the QR code, or the whole footer, is left out and a note says why. Exports do not include the footer.",
        "El tamaño de la etiqueta y el área imprimible nunca cambian. Si ese lugar no está en blanco o queda fuera del área imprimible, se omite el código QR, o todo el pie, y una nota explica por qué. Las exportaciones no incluyen el pie.",
    ),
    (
        "Put the whole footer, QR code included, at the bottom-left",
        "Colocar todo el pie, con el código QR, abajo a la izquierda",
    ),
    (
        "Put the whole footer, QR code included, at the bottom center",
        "Colocar todo el pie, con el código QR, abajo en el centro",
    ),
    (
        "Put the whole footer, QR code included, at the bottom-right",
        "Colocar todo el pie, con el código QR, abajo a la derecha",
    ),
    (
        "The footer fits: the canvas shows it where it will print.",
        "El pie cabe: el lienzo lo muestra donde se imprimirá.",
    ),
    ("Footer", "Pie"),
    (
        "Show the label footer settings in the properties panel",
        "Mostrar los ajustes del pie de etiqueta en el panel de propiedades",
    ),
    (
        "Print is off while a barcode or QR code overlaps another object. Move or resize them so they no longer touch, and Print turns back on.",
        "Imprimir está desactivado mientras un código de barras o QR se superpone a otro objeto. Muévelos o cambia su tamaño para que no se toquen, y Imprimir vuelve a activarse.",
    ),
    (
        "Overlap: these codes touch another object, so Print is off until they are apart. Move or resize them:",
        "Superposición: estos códigos tocan otro objeto, así que Imprimir está desactivado hasta que se separen. Muévelos o cambia su tamaño:",
    ),
    ("This side up", "Este lado hacia arriba"),
    ("Keep dry", "Mantener seco"),
    ("Recycling", "Reciclaje"),
    ("Hazard", "Peligro"),
    ("Do not stack", "No apilar"),
    ("Keep cool", "Mantener fresco"),
    ("Keep out of sunlight", "Proteger de la luz solar"),
    ("Package", "Paquete"),
    // Interface reorganization
    (
        "Check every barcode and QR code for content, size, clear space, overlap, and clipping",
        "Comprobar el contenido, el tamaño, el margen libre, las superposiciones y los recortes de cada código de barras y QR",
    ),
    (
        "Choose a printer on the Printer tab first. Stock profiles belong to a printer.",
        "Elige primero una impresora en la pestaña Impresora. Los perfiles de etiquetas pertenecen a una impresora.",
    ),
    ("Compact layout", "Diseño compacto"),
    ("Hide other notes", "Ocultar las demás notas"),
    ("Label footer…", "Pie de etiqueta…"),
    ("Label footer (on)…", "Pie de etiqueta (activado)…"),
    (
        "Labels you print are listed here, so you can open or reprint them.",
        "Las etiquetas que imprimes aparecen aquí para que puedas abrirlas o reimprimirlas.",
    ),
    ("More notes", "Más notas"),
    (
        "Tighter spacing, so more fits on a small screen",
        "Espaciado más ajustado para que quepa más en una pantalla pequeña",
    ),
    ("Zoom to 100%", "Zoom al 100%"),
    ("Printer", "Impresora"),
    ("Automation", "Automatización"),
    (
        "Place copies of this label on one page of any paper size your printer supports, for an office printer or a sheet of sticky labels. ${serial} counts up from one label to the next.",
        "Coloca copias de esta etiqueta en una página de cualquier tamaño que admita tu impresora, para una impresora de oficina o una hoja de etiquetas adhesivas. ${serial} aumenta de una etiqueta a la siguiente.",
    ),
    ("Printer paper size", "Tamaño de papel de la impresora"),
    ("Custom size", "Tamaño personalizado"),
    (
        "Place several labels on one page of any paper size",
        "Coloca varias etiquetas en una página de cualquier tamaño",
    ),
    (
        "Choose the paper or labels loaded in the printer. Custom uses the output dimensions.",
        "Elige el papel o las etiquetas cargadas en la impresora. Personalizado usa las dimensiones de salida.",
    ),
    (
        "Choose a printer and paper size, check alignment, and save it all as a stock profile you can load later.",
        "Elige una impresora y un tamaño de papel, comprueba la alineación y guárdalo todo como un perfil que podrás cargar después.",
    ),
    ("Use the paper size", "Usar el tamaño del papel"),
    ("Choose printer", "Elegir impresora"),
    ("Looking for printers…", "Buscando impresoras…"),
    (
        "No printers found. Add a printer in your system settings, then refresh.",
        "No se encontraron impresoras. Añade una en la configuración del sistema y actualiza.",
    ),
    ("Choose a printer first.", "Elige primero una impresora."),
    ("Paper or label size", "Tamaño de papel o etiqueta"),
    ("Custom dimensions", "Dimensiones personalizadas"),
    (
        "This printer does not list its paper sizes. Configure them in its driver.",
        "Esta impresora no indica sus tamaños de papel. Configúralos en su controlador.",
    ),
    ("Saves:", "Guarda:"),
    (
        "For example: 4 × 6 shipping labels",
        "Por ejemplo: etiquetas de envío 4 × 6",
    ),
    ("Replace stock profile", "Reemplazar perfil de etiquetas"),
    ("Stock profile saved", "Perfil de etiquetas guardado"),
    (
        "A profile with this name already exists for this printer. Saving replaces it.",
        "Ya existe un perfil con este nombre para esta impresora. Al guardar se reemplaza.",
    ),
    (
        "You have reached the limit of saved stock profiles. Delete one to add another.",
        "Has alcanzado el límite de perfiles guardados. Elimina uno para añadir otro.",
    ),
    (
        "A stock profile remembers a printer with its paper size, resolution, and alignment. Load one to switch to all of it at once.",
        "Un perfil de etiquetas recuerda una impresora con su tamaño de papel, resolución y alineación. Cárgalo para cambiar todo a la vez.",
    ),
    (
        "No saved stock yet. Set up the printer and paper, then save them below.",
        "Aún no hay perfiles guardados. Configura la impresora y el papel y guárdalos abajo.",
    ),
    ("Load", "Cargar"),
    (
        "This printer is not connected",
        "Esta impresora no está conectada",
    ),
    ("Delete stock profile?", "¿Eliminar el perfil de etiquetas?"),
    ("Stock profile deleted", "Perfil de etiquetas eliminado"),
    (
        "Choose the printer you will print on.",
        "Elige la impresora en la que vas a imprimir.",
    ),
    (
        "Choose the size loaded in the printer. Label printers list their label sizes; office printers list every paper size they support, such as Letter, Legal, or A4.",
        "Elige el tamaño cargado en la impresora. Las impresoras de etiquetas muestran sus tamaños de etiqueta; las de oficina, todos los tamaños de papel que admiten, como Carta, Oficio o A4.",
    ),
    (
        "Optional. Print one test page and compare its 1 mm border to the edges. If it is off, move it with the offsets.",
        "Opcional. Imprime una página de prueba y compara su borde de 1 mm con los bordes. Si está desplazado, muévelo con los desplazamientos.",
    ),
    (
        "Name this setup so you can switch back to it in one click from Label stock.",
        "Ponle nombre a esta configuración para volver a ella con un clic desde Papel de etiquetas.",
    ),
    (
        "It is now the active profile. Switch profiles any time from Active profile at the top of Print Center.",
        "Ahora es el perfil activo. Cambia de perfil cuando quieras desde Perfil activo, arriba en el Centro de impresión.",
    ),
    ("Finish", "Terminar"),
    ("Set up another", "Configurar otro"),
    ("Active profile:", "Perfil activo:"),
    ("Stock profile updated", "Perfil de etiquetas actualizado"),
    ("None", "Ninguno"),
    (
        "Changed since it was loaded",
        "Modificado desde que se cargó",
    ),
    ("Update profile", "Actualizar perfil"),
    ("Active", "Activo"),
    (
        "Save the current printer, paper, resolution, and alignment into this profile",
        "Guarda en este perfil la impresora, el papel, la resolución y la alineación actuales",
    ),
    (
        "Changed since it was loaded. Update profile saves the changes.",
        "Modificado desde que se cargó. Actualizar perfil guarda los cambios.",
    ),
    ("Saved and active:", "Guardado y activo:"),
    (
        "Save the current setup as a new profile",
        "Guardar la configuración actual como un perfil nuevo",
    ),
    ("Unsaved", "Sin guardar"),
    ("rotated printing", "impresión girada"),
    ("{count} objects", "{count} objetos"),
    ("1 copy", "1 copia"),
    ("{count} copies", "{count} copias"),
    ("Printer media: {media}", "Soporte de la impresora: {media}"),
    ("Not selected", "Sin seleccionar"),
    (
        "Custom — matches output",
        "Personalizado — coincide con la salida",
    ),
    (
        "{count} imported labels queued",
        "{count} etiquetas importadas en cola",
    ),
    (
        "Print queue · {count} labels",
        "Cola de impresión · {count} etiquetas",
    ),
    ("Label {n} of {total}", "Etiqueta {n} de {total}"),
    ("Queued labels", "Etiquetas en cola"),
    (
        "Print all {count} labels in order",
        "Imprimir las {count} etiquetas en orden",
    ),
    ("Recently opened", "Abiertos recientemente"),
    ("of {total}", "de {total}"),
    (
        "Advanced layout & resolution",
        "Diseño y resolución avanzados",
    ),
    (
        "Demo workspace · All sample labels are fictional",
        "Espacio de demostración · Todas las etiquetas de ejemplo son ficticias",
    ),
    ("Automatic", "Automático"),
    ("Manual", "Manual"),
    ("Full page", "Página completa"),
    ("Preview", "Vista previa"),
    ("01  EXTRACT", "01  EXTRAER"),
    ("02  PAPER & LAYOUT", "02  PAPEL Y DISEÑO"),
    ("03  PRINT & EXPORT", "03  IMPRIMIR Y EXPORTAR"),
    (
        "Automatically detect the label region",
        "Detectar automáticamente la zona de la etiqueta",
    ),
    (
        "Manually adjust the crop on the original document",
        "Ajustar manualmente el recorte en el documento original",
    ),
    (
        "Use the entire page without cropping",
        "Usar la página completa sin recortar",
    ),
    (
        "Paper boundary · Final output uses the full-resolution render",
        "Límite del papel · La salida final usa el renderizado a resolución completa",
    ),
    (
        "Original document · Green boundary is the crop · Crop before rotating",
        "Documento original · El borde verde es el recorte · Recorta antes de girar",
    ),
    (
        "Likely thermal label printer",
        "Probablemente una impresora térmica de etiquetas",
    ),
    (
        "Likely label printer · check media",
        "Probablemente una impresora de etiquetas · revisa el soporte",
    ),
    (
        "Supports label-sized media · verify printer type",
        "Admite soportes del tamaño de etiqueta · verifica el tipo de impresora",
    ),
    (
        "Printer type unknown · check its driver and paper",
        "Tipo de impresora desconocido · revisa su controlador y el papel",
    ),
    ("Margins & image enhancement", "Márgenes y mejora de imagen"),
    (
        "Print through the printer driver",
        "Imprimir a través del controlador de la impresora",
    ),
    (
        "Recommended. The printer's driver handles the label size and printing.",
        "Recomendado. El controlador de la impresora gestiona el tamaño de la etiqueta y la impresión.",
    ),
    ("Print using raw TSPL", "Imprimir con TSPL sin procesar"),
    (
        "Raw TSPL: verify printer compatibility and loaded label size.",
        "TSPL sin procesar: verifica la compatibilidad de la impresora y el tamaño de etiqueta cargado.",
    ),
    (
        "For TSPL printers such as many 4 × 6 shipping printers. Requires a raw CUPS queue. The printer keeps its own gap calibration.",
        "Para impresoras TSPL, como muchas impresoras de envío de 4 × 6. Requiere una cola CUPS sin procesar. La impresora conserva su propia calibración de separación.",
    ),
    (
        "For TSPL printers such as many 4 × 6 shipping printers. Requires a printer that accepts RAW jobs. The printer keeps its own gap calibration.",
        "Para impresoras TSPL, como muchas impresoras de envío de 4 × 6. Requiere una impresora que acepte trabajos RAW. La impresora conserva su propia calibración de separación.",
    ),
];
const DE: &[(&str, &str)] = &[
    ("No matches", "Keine Treffer"),
    ("Open ThermalPrint", "ThermalPrint öffnen"),
    ("Check barcodes", "Barcodes prüfen"),
    ("Background printing", "Drucken im Hintergrund"),
    (
        "Keep running when the window is closed",
        "Nach dem Schließen des Fensters weiterlaufen",
    ),
    (
        "The tray menu opens ThermalPrint, pauses automatic printing, or quits. Your computer must be awake.",
        "Über das Infobereich-Menü können Sie ThermalPrint öffnen, den automatischen Druck pausieren oder beenden. Der Computer muss eingeschaltet sein.",
    ),
    ("Start at login", "Bei der Anmeldung starten"),
    (
        "Automatic printing checks file contents to prevent repeat prints, even after a file is renamed.",
        "Der automatische Druck prüft den Dateiinhalt, um doppelte Ausdrucke auch nach dem Umbenennen zu verhindern.",
    ),
    (
        "Clear duplicate protection…",
        "Duplikatschutz zurücksetzen…",
    ),
    (
        "Clear duplicate protection?",
        "Duplikatschutz zurücksetzen?",
    ),
    (
        "Previously printed files can print automatically again if they arrive in the watch folder.",
        "Bereits gedruckte Dateien können erneut automatisch gedruckt werden, wenn sie im überwachten Ordner eintreffen.",
    ),
    ("Clear duplicate protection", "Duplikatschutz zurücksetzen"),
    ("Printer setup assistant", "Druckereinrichtungsassistent"),
    (
        "Set up printer and label stock",
        "Drucker und Etikettenmaterial einrichten",
    ),
    ("Next", "Weiter"),
    ("Print one test label", "Ein Testetikett drucken"),
    ("Close assistant", "Assistent schließen"),
    ("Saved label stock", "Gespeichertes Etikettenmaterial"),
    ("Stock profile name", "Name des Materialprofils"),
    ("Save stock profile", "Materialprofil speichern"),
    ("Reprint…", "Erneut drucken…"),
    (
        "Review and reprint with the recorded settings",
        "Mit den gespeicherten Einstellungen prüfen und erneut drucken",
    ),
    ("Reprint label?", "Etikett erneut drucken?"),
    (
        "This sends another print job using the recorded settings.",
        "Dies sendet einen weiteren Druckauftrag mit den gespeicherten Einstellungen.",
    ),
    ("Reprint", "Erneut drucken"),
    ("Cable flag", "Kabelfahne"),
    (
        "Matching cable IDs on both sides of a fold",
        "Gleiche Kabelkennung auf beiden Seiten der Falz",
    ),
    ("Price and barcode", "Preis und Barcode"),
    (
        "Product, price, and scannable stock code",
        "Produkt, Preis und scanbarer Artikelcode",
    ),
    ("Shelf barcode", "Regalbarcode"),
    (
        "Shelf location with a stock barcode",
        "Regalplatz mit Artikelbarcode",
    ),
    ("Small product QR", "Kleiner Produkt-QR-Code"),
    (
        "Compact product details and QR code",
        "Kompakte Produktangaben und QR-Code",
    ),
    ("Printing", "Druckt"),
    (
        "Print labels automatically?",
        "Etiketten automatisch drucken?",
    ),
    (
        "Every label that lands in the watch folder will print by itself, without you pressing Print. A folder used only for labels is safest.",
        "Jedes Etikett, das im überwachten Ordner landet, wird von selbst gedruckt, ohne dass Sie auf Drucken klicken. Am sichersten ist ein Ordner, der nur für Etiketten verwendet wird.",
    ),
    (
        "This looks like your Downloads folder: an invoice or any other PDF saved there could print too. A separate folder just for labels is safer.",
        "Das sieht nach Ihrem Download-Ordner aus: Eine Rechnung oder jedes andere dort gespeicherte PDF könnte ebenfalls gedruckt werden. Ein eigener Ordner nur für Etiketten ist sicherer.",
    ),
    (
        "Only single-page labels whose crop needs no review print. Each one waits 5 seconds with a Cancel button, and Pause stops everything.",
        "Es werden nur einseitige Etiketten gedruckt, deren Zuschnitt keine Prüfung braucht. Jedes wartet 5 Sekunden mit einer Schaltfläche „Abbrechen“, und „Pause“ stoppt alles.",
    ),
    ("Turn on", "Einschalten"),
    ("auto-print paused", "Auto-Druck pausiert"),
    ("auto-print on", "Auto-Druck an"),
    ("Resume auto-print", "Auto-Druck fortsetzen"),
    ("Pause auto-print", "Auto-Druck pausieren"),
    (
        "Stop or restart printing labels by themselves. Waiting labels stay in the queue.",
        "Stoppt oder startet das automatische Drucken von Etiketten. Wartende Etiketten bleiben in der Warteschlange.",
    ),
    (
        "Print labels automatically",
        "Etiketten automatisch drucken",
    ),
    (
        "Print labels that land in this folder by themselves. Only single-page labels whose crop needs no review print; the rest wait in the queue. Off until you turn it on.",
        "Druckt Etiketten, die in diesem Ordner landen, von selbst. Es werden nur einseitige Etiketten gedruckt, deren Zuschnitt keine Prüfung braucht; die übrigen warten in der Warteschlange. Aus, bis Sie es einschalten.",
    ),
    ("Only files named", "Nur Dateien namens"),
    (
        "Leave empty to print every label. Use * for any letters and ? for one letter; separate several names with commas.",
        "Leer lassen, um jedes Etikett zu drucken. * steht für beliebige Zeichen, ? für ein Zeichen; mehrere Namen mit Kommas trennen.",
    ),
    (
        "File names that print automatically",
        "Dateinamen, die automatisch gedruckt werden",
    ),
    ("Most per minute", "Höchstens pro Minute"),
    (
        "A burst of files cannot print more labels than this in one minute; the rest wait.",
        "Eine Flut von Dateien kann nicht mehr Etiketten als diese Zahl pro Minute drucken; der Rest wartet.",
    ),
    (
        "Most labels printed automatically per minute",
        "Höchstzahl automatisch gedruckter Etiketten pro Minute",
    ),
    ("Safe margin", "Sicherheitsrand"),
    (
        "Keep content this far from the label's edge. It is shaded on the canvas and objects inside it are flagged, even when the printer's driver reports no unprintable border. It is remembered for each printer.",
        "Halten Sie den Inhalt so weit vom Etikettenrand entfernt. Der Bereich wird auf der Zeichenfläche schattiert, und Objekte darin werden markiert, auch wenn der Druckertreiber keinen nicht bedruckbaren Rand meldet. Er wird für jeden Drucker gespeichert.",
    ),
    (
        "Safe margin in millimeters",
        "Sicherheitsrand in Millimetern",
    ),
    (
        "These objects reach into the unprintable border or your safe margin and may be cut off:",
        "Diese Objekte reichen in den nicht bedruckbaren Rand oder Ihren Sicherheitsrand und können abgeschnitten werden:",
    ),
    (
        "Choose a printer in Print Center to see its unprintable border, or set a safe margin in the View menu.",
        "Wählen Sie im Druckcenter einen Drucker, um dessen nicht bedruckbaren Rand zu sehen, oder legen Sie im Menü Ansicht einen Sicherheitsrand fest.",
    ),
    (
        "Select printer media in Print Center to see its unprintable border, or set a safe margin in the View menu.",
        "Wählen Sie im Druckcenter das Druckmedium, um den nicht bedruckbaren Rand zu sehen, oder legen Sie im Menü Ansicht einen Sicherheitsrand fest.",
    ),
    (
        "Choose media that matches this label's size to see the printer's unprintable border, or set a safe margin in the View menu.",
        "Wählen Sie ein Medium passend zur Etikettengröße, um den nicht bedruckbaren Rand des Druckers zu sehen, oder legen Sie im Menü Ansicht einen Sicherheitsrand fest.",
    ),
    (
        "The printer's driver reports no unprintable border for this media. Set a safe margin in the View menu to keep content away from the edge.",
        "Der Druckertreiber meldet für dieses Medium keinen nicht bedruckbaren Rand. Legen Sie im Menü Ansicht einen Sicherheitsrand fest, um Inhalte vom Rand fernzuhalten.",
    ),
    (
        "Keeps designs this far from the label's edge. Use it when your printer cuts off the edges but its driver reports no unprintable border. It is remembered for each printer.",
        "Hält Entwürfe so weit vom Etikettenrand entfernt. Nutzen Sie dies, wenn Ihr Drucker die Ränder abschneidet, der Treiber aber keinen nicht bedruckbaren Rand meldet. Wird für jeden Drucker gespeichert.",
    ),
    ("Logo in the center", "Logo in der Mitte"),
    (
        "A logo hides part of the code. ThermalPrint keeps it small enough, and uses the highest error correction, so phones can still scan it.",
        "Ein Logo verdeckt einen Teil des Codes. ThermalPrint hält es klein genug und nutzt die höchste Fehlerkorrektur, damit Smartphones den Code weiterhin scannen können.",
    ),
    ("Add logo…", "Logo hinzufügen…"),
    ("Change logo…", "Logo ändern…"),
    ("Remove logo", "Logo entfernen"),
    ("Choose a saved logo", "Gespeichertes Logo wählen"),
    ("Logo size", "Logogröße"),
    (
        "The share of the code's width that is cleared for the logo",
        "Der Anteil der Codebreite, der für das Logo freigehalten wird",
    ),
    (
        "Largest size that scans reliably:",
        "Größte Größe, die zuverlässig scannt:",
    ),
    (
        "Error correction is set to High while a logo is used.",
        "Die Fehlerkorrektur ist mit Logo auf „Hoch“ eingestellt.",
    ),
    (
        "The code is small for a logo. Make it at least this wide:",
        "Der Code ist für ein Logo zu klein. Machen Sie ihn mindestens so breit:",
    ),
    ("No printer selected", "Kein Drucker ausgewählt"),
    (
        "No recent documents yet. Labels you open will appear here.",
        "Noch keine zuletzt geöffneten Dokumente. Etiketten, die Sie öffnen, erscheinen hier.",
    ),
    ("Reviewed", "Geprüft"),
    (
        "Print this label (Ctrl+P)",
        "Dieses Etikett drucken (Strg+P)",
    ),
    ("Add a label first", "Fügen Sie zuerst ein Etikett hinzu"),
    (
        "The preview is still being prepared",
        "Die Vorschau wird noch erstellt",
    ),
    (
        "The label would be clipped. Adjust the layout",
        "Das Etikett würde abgeschnitten. Passen Sie das Layout an",
    ),
    (
        "Tick Reviewed once you have checked the crop",
        "Setzen Sie Geprüft, sobald Sie den Zuschnitt kontrolliert haben",
    ),
    (
        "Wait for the current operation to finish",
        "Warten Sie, bis der aktuelle Vorgang beendet ist",
    ),
    ("Undo", "Rückgängig"),
    ("Dismiss", "Schließen"),
    ("Watching", "Überwacht:"),
    ("Hide this", "Ausblenden"),
    ("Turn watching back on", "Überwachung wieder einschalten"),
    ("Recent messages", "Letzte Meldungen"),
    ("Show recent messages", "Letzte Meldungen anzeigen"),
    ("Add from Processed…", "Aus Processed hinzufügen…"),
    (
        "Keep going if a label fails",
        "Bei einem Fehler mit dem Rest fortfahren",
    ),
    ("Open Import & Convert", "Importieren öffnen"),
    ("Messages", "Meldungen"),
    ("Will print", "Wird gedruckt"),
    (
        "Labels added from now on count as reviewed, so Print all does not wait for you to check each crop. Turn this on only for labels whose layout you trust.",
        "Ab jetzt hinzugefügte Etiketten gelten als geprüft, sodass Alle drucken nicht auf die Prüfung jedes Zuschnitts wartet. Nur für Etiketten einschalten, deren Layout Sie kennen.",
    ),
    (
        "Choose labels from the Processed folder to print them again",
        "Etiketten aus dem Ordner Processed auswählen, um sie erneut zu drucken",
    ),
    (
        "Print all skips a label that fails and continues with the rest. Failed labels stay in the queue.",
        "Alle drucken überspringt ein fehlgeschlagenes Etikett und macht mit den übrigen weiter. Fehlgeschlagene Etiketten bleiben in der Warteschlange.",
    ),
    ("Nothing yet", "Noch nichts"),
    ("Current printer", "Aktueller Drucker"),
    (
        "Printer for watched labels",
        "Drucker für überwachte Etiketten",
    ),
    (
        "Skip review for automatic crops",
        "Prüfung bei automatischem Zuschnitt überspringen",
    ),
    ("Get ready to print", "Druck vorbereiten"),
    (
        "Choose your label printer in Import & Convert",
        "Wählen Sie Ihren Etikettendrucker unter Importieren",
    ),
    (
        "Add your label printer in your system settings, then reopen the app",
        "Fügen Sie Ihren Etikettendrucker in den Systemeinstellungen hinzu und öffnen Sie die App erneut",
    ),
    (
        "Choose the label size your printer has loaded",
        "Wählen Sie die im Drucker eingelegte Etikettengröße",
    ),
    (
        "Optional: watch a folder such as Downloads so new labels appear by themselves",
        "Optional: Überwachen Sie einen Ordner wie Downloads, damit neue Etiketten von selbst erscheinen",
    ),
    (
        "Restore your last queue?",
        "Letzte Warteschlange wiederherstellen?",
    ),
    (
        "Move printed labels to a Processed folder",
        "Gedruckte Etiketten in einen Ordner Processed verschieben",
    ),
    (
        "After printing, labels from the watch folder are removed from the queue and moved into a Processed folder inside it. Labels added from elsewhere are not moved.",
        "Nach dem Drucken werden Etiketten aus dem überwachten Ordner aus der Warteschlange entfernt und in einen Ordner Processed darin verschoben. Von anderswo hinzugefügte Etiketten werden nicht verschoben.",
    ),
    ("About ThermalPrint", "Über ThermalPrint"),
    ("Add labels…", "Etiketten hinzufügen…"),
    ("Advanced driver resolution", "Erweiterte Treiberauflösung"),
    ("Copies", "Kopien"),
    ("Height", "Höhe"),
    ("Width", "Breite"),
    ("W", "B"),
    ("H", "H"),
    (
        "Import PDF or image labels (Ctrl+O)",
        "PDF- oder Bild-Etiketten importieren (Strg+O)",
    ),
    ("Printing service: CUPS", "Druckdienst: CUPS"),
    (
        "Refresh available CUPS printers",
        "Verfügbare CUPS-Drucker aktualisieren",
    ),
    ("Rendering DPI", "Render-DPI"),
    ("Save", "Speichern"),
    ("Serial variable", "Seriennummern-Variable"),
    (
        "Swap label width and height",
        "Breite und Höhe des Etiketts tauschen",
    ),
    ("Swap portrait / landscape", "Hoch-/Querformat tauschen"),
    ("Templates", "Vorlagen"),
    ("Zoom in", "Vergrößern"),
    ("Zoom out", "Verkleinern"),
    (
        "A successful submission means CUPS accepted the job; check physical output and barcode readability.",
        "Eine erfolgreiche Übergabe bedeutet, dass CUPS den Auftrag akzeptiert hat; prüfen Sie den Ausdruck und die Lesbarkeit des Barcodes.",
    ),
    ("Actual size", "Tatsächliche Größe"),
    (
        "Add new PDF and image labels automatically when they appear in a folder, such as Downloads. Files already there when you turn this on are ignored; labels saved while ThermalPrint is closed are added when it opens.",
        "Neue PDF- und Bildetiketten automatisch hinzufügen, sobald sie in einem Ordner wie Downloads erscheinen. Dateien, die beim Einschalten schon da sind, werden ignoriert; Etiketten, die bei geschlossenem ThermalPrint gespeichert wurden, kommen beim Öffnen hinzu.",
    ),
    ("Alignment", "Ausrichtung"),
    ("Back", "Zurück"),
    ("Backward", "Nach hinten"),
    (
        "Barcode or QR code content",
        "Inhalt des Barcodes oder QR-Codes",
    ),
    (
        "Black-and-white threshold (advanced)",
        "Schwarz-Weiß-Schwelle (erweitert)",
    ),
    ("Bold", "Fett"),
    ("Cancel", "Abbrechen"),
    ("Center", "Zentriert"),
    ("Check CUPS job status", "CUPS-Auftragsstatus prüfen"),
    (
        "Check jobs submitted to the selected CUPS printer",
        "Prüft die an den gewählten CUPS-Drucker gesendeten Aufträge",
    ),
    (
        "Check jobs submitted to the selected printer",
        "Prüft die an den gewählten Drucker gesendeten Aufträge",
    ),
    (
        "Check the original crop, then confirm it in the left panel.",
        "Prüfen Sie den ursprünglichen Zuschnitt und bestätigen Sie ihn im linken Bereich.",
    ),
    ("Choose CSV file…", "CSV-Datei wählen…"),
    (
        "Choose a printer in Print Center first.",
        "Wählen Sie zuerst im Druckcenter einen Drucker.",
    ),
    (
        "Choose a printer in Print Center to print this design. Export is available without a printer.",
        "Wählen Sie im Druckcenter einen Drucker, um dieses Design zu drucken. Der Export funktioniert auch ohne Drucker.",
    ),
    ("Choose folder…", "Ordner wählen…"),
    ("Clear", "Entfernen"),
    ("Clear history", "Verlauf löschen"),
    ("Close", "Schließen"),
    ("Copy version information", "Versionsinformationen kopieren"),
    ("Credits and license", "Danksagungen und Lizenz"),
    ("Custom scale", "Benutzerdefinierte Skalierung"),
    (
        "Default processing preserves tones. Thresholding can damage fine barcode detail.",
        "Die Standardverarbeitung erhält Tonwerte. Die Schwellenwertbildung kann feine Barcode-Details beschädigen.",
    ),
    ("Delete", "Löschen"),
    (
        "Delete all selected objects",
        "Alle ausgewählten Objekte löschen",
    ),
    ("Delete selected", "Auswahl löschen"),
    ("Delete selected objects", "Ausgewählte Objekte löschen"),
    (
        "Design a custom label or prepare a downloaded shipping label.",
        "Gestalten Sie ein eigenes Etikett oder bereiten Sie ein heruntergeladenes Versandetikett vor.",
    ),
    (
        "Design and prepare thermal labels for Linux.",
        "Thermoetiketten für Linux gestalten und vorbereiten.",
    ),
    ("Discard", "Verwerfen"),
    (
        "Drag to draw. Drag inside to move; drag a corner to resize. Crop coordinates refer to the original page.",
        "Ziehen zum Zeichnen. Im Rahmen ziehen zum Verschieben, an einer Ecke ziehen zum Skalieren. Die Zuschnittkoordinaten beziehen sich auf die Originalseite.",
    ),
    (
        "Drag to move · square to resize · circle to rotate · Shift: 15°",
        "Ziehen zum Verschieben · Quadrat zum Skalieren · Kreis zum Drehen · Umschalt: 15°",
    ),
    (
        "Drag to rotate freely. Hold Shift to snap to 15°.",
        "Ziehen zum freien Drehen. Mit Umschalt rasten Sie in 15°-Schritten ein.",
    ),
    (
        "Driver default resolution",
        "Standardauflösung des Treibers",
    ),
    (
        "Drop PDFs or images to build a print queue",
        "PDFs oder Bilder hierher ziehen, um eine Druckwarteschlange zu erstellen",
    ),
    (
        "Drop a downloaded label anywhere.",
        "Ziehen Sie ein heruntergeladenes Etikett einfach irgendwohin.",
    ),
    ("Duplicate", "Duplizieren"),
    (
        "Duplicate selected objects (Ctrl+D)",
        "Ausgewählte Objekte duplizieren (Strg+D)",
    ),
    (
        "Embedded image. Resize with W/H or the canvas handle.",
        "Eingebettetes Bild. Größe mit B/H oder dem Griff auf der Zeichenfläche ändern.",
    ),
    (
        "Enter your real product code or reference",
        "Geben Sie Ihren echten Produktcode oder Ihre Referenz ein",
    ),
    (
        "Every row is checked first; nothing prints if a row has a problem.",
        "Zuerst wird jede Zeile geprüft; bei einem Problem wird nichts gedruckt.",
    ),
    ("Explore ThermalPrint", "ThermalPrint entdecken"),
    (
        "Export a CSV from your spreadsheet. Column names are not case-sensitive; ${serial} counts up from the design's serial number.",
        "Exportieren Sie eine CSV-Datei aus Ihrer Tabelle. Spaltennamen unterscheiden nicht zwischen Groß- und Kleinschreibung; ${serial} zählt ab der Seriennummer des Designs hoch.",
    ),
    ("Export", "Exportieren"),
    (
        "Export the prepared label as PDF",
        "Das vorbereitete Etikett als PDF exportieren",
    ),
    (
        "Export the prepared label as PNG",
        "Das vorbereitete Etikett als PNG exportieren",
    ),
    ("File", "Datei"),
    ("Filled", "Gefüllt"),
    (
        "Fit label to canvas and reset panning",
        "Etikett an Zeichenfläche anpassen und Verschiebung zurücksetzen",
    ),
    (
        "Fit label to the preview area",
        "Etikett an den Vorschaubereich anpassen",
    ),
    ("Fit within paper", "In Papier einpassen"),
    ("Focus canvas", "Zeichenfläche fokussieren"),
    ("Font size in points", "Schriftgröße in Punkt"),
    ("Forward", "Nach vorn"),
    (
        "From any marketplace or carrier. No account needed.",
        "Von jedem Marktplatz oder Versanddienstleister. Kein Konto nötig.",
    ),
    (
        "Hide navigation and properties to expand the canvas",
        "Navigation und Eigenschaften ausblenden, um die Zeichenfläche zu vergrößern",
    ),
    ("Horizontal", "Horizontal"),
    (
        "Horizontal print offset in millimeters",
        "Horizontaler Druckversatz in Millimetern",
    ),
    (
        "I checked the crop and barcode edges",
        "Ich habe Zuschnitt und Barcode-Ränder geprüft",
    ),
    (
        "If the loaded paper matches the printer media, match the output below. Otherwise select media that matches the label size you want.",
        "Wenn das eingelegte Papier dem Druckermedium entspricht, passen Sie die Ausgabe unten an. Wählen Sie andernfalls ein Medium passend zur gewünschten Etikettengröße.",
    ),
    (
        "If the loaded paper matches the printer media, use the button below. To keep the current label size, choose matching media in Print Center instead.",
        "Wenn das eingelegte Papier dem Druckermedium entspricht, verwenden Sie die Schaltfläche unten. Um die aktuelle Etikettengröße beizubehalten, wählen Sie stattdessen im Druckcenter ein passendes Medium.",
    ),
    ("Job status", "Auftragsstatus"),
    ("Label", "Etikett"),
    ("Label height in millimeters", "Etiketthöhe in Millimetern"),
    ("Label name", "Etikettenname"),
    (
        "Label setup (no selection)",
        "Etiketteneinrichtung (keine Auswahl)",
    ),
    ("Label stock", "Etikettenmaterial"),
    (
        "Label width in millimeters",
        "Etikettenbreite in Millimetern",
    ),
    ("Labels, ready to go.", "Etiketten, startklar."),
    ("Left", "Links"),
    ("MIT License", "MIT-Lizenz"),
    ("Main menu", "Hauptmenü"),
    (
        "Match label to printer media",
        "Etikett an Druckermedium anpassen",
    ),
    (
        "Match output to printer media",
        "Ausgabe an Druckermedium anpassen",
    ),
    ("More from the author", "Mehr vom Autor"),
    (
        "Move label earlier in the print queue",
        "Etikett in der Druckwarteschlange nach vorn verschieben",
    ),
    (
        "Move label later in the print queue",
        "Etikett in der Druckwarteschlange nach hinten verschieben",
    ),
    (
        "Move object one layer backward",
        "Objekt eine Ebene nach hinten",
    ),
    (
        "Move object one layer forward",
        "Objekt eine Ebene nach vorn",
    ),
    ("New label", "Neues Etikett"),
    ("No folder chosen", "Kein Ordner gewählt"),
    (
        "No printer selected. Export is available without a printer.",
        "Kein Drucker gewählt. Der Export funktioniert auch ohne Drucker.",
    ),
    ("Object", "Objekt"),
    ("Object height in millimeters", "Objekthöhe in Millimetern"),
    (
        "Object horizontal position in millimeters",
        "Horizontale Objektposition in Millimetern",
    ),
    ("Object text", "Objekttext"),
    (
        "Object vertical position in millimeters",
        "Vertikale Objektposition in Millimetern",
    ),
    ("Object width in millimeters", "Objektbreite in Millimetern"),
    ("Open", "Öffnen"),
    (
        "Open a saved label design",
        "Ein gespeichertes Etikettendesign öffnen",
    ),
    ("Open import queue", "Importwarteschlange öffnen"),
    (
        "Open imported labels for review",
        "Importierte Etiketten zur Prüfung öffnen",
    ),
    (
        "Open this label for review",
        "Dieses Etikett zur Prüfung öffnen",
    ),
    ("Original / crop", "Original / Zuschnitt"),
    (
        "Output label proportions",
        "Proportionen des Ausgabeetiketts",
    ),
    ("Output label size", "Größe des Ausgabeetiketts"),
    ("Output settings", "Ausgabeeinstellungen"),
    ("Page", "Seite"),
    ("Prepare your label", "Bereiten Sie Ihr Etikett vor"),
    ("Preview row", "Vorschau der Zeile"),
    ("Preview row number", "Zeilennummer der Vorschau"),
    (
        "Preview the final label as it will print",
        "Vorschau des fertigen Etiketts, wie es gedruckt wird",
    ),
    ("Print", "Drucken"),
    ("Print Center", "Druckcenter"),
    ("Print alignment test", "Ausrichtungstest drucken"),
    ("Print label   Ctrl+P", "Etikett drucken   Strg+P"),
    (
        "Print label using the selected printer",
        "Etikett mit dem gewählten Drucker drucken",
    ),
    (
        "Print one label per row of a spreadsheet, filling ${column} placeholders",
        "Pro Tabellenzeile ein Etikett drucken und die ${column}-Platzhalter füllen",
    ),
    ("Print using raw ZPL", "Mit Roh-ZPL drucken"),
    ("Printer & media", "Drucker und Medien"),
    ("Printer queue", "Druckerwarteschlange"),
    ("Printer resolution", "Druckerauflösung"),
    ("Printer status", "Druckerstatus"),
    ("Printing terms", "Druckbegriffe"),
    (
        "Prints a ruler label with a 1 mm border so you can measure the offset",
        "Druckt ein Lineal-Etikett mit 1-mm-Rand, um den Versatz zu messen",
    ),
    ("Properties", "Eigenschaften"),
    ("Properties panel", "Eigenschaftenbereich"),
    (
        "Put ${column} placeholders in text, barcode, and QR content. Each row of your spreadsheet becomes one label.",
        "Setzen Sie ${column}-Platzhalter in Text, Barcode und QR-Inhalt. Jede Zeile Ihrer Tabelle wird zu einem Etikett.",
    ),
    ("QR code content", "QR-Code-Inhalt"),
    ("QR content", "QR-Inhalt"),
    ("Queue & diagnostics", "Warteschlange und Diagnose"),
    ("Quit", "Beenden"),
    (
        "Raw ZPL: verify printer compatibility and loaded label size.",
        "Roh-ZPL: Druckerkompatibilität und eingelegte Etikettengröße prüfen.",
    ),
    ("Recent documents", "Zuletzt verwendet"),
    ("Recent prints", "Letzte Drucke"),
    ("Redo (Ctrl+Y)", "Wiederholen (Strg+Y)"),
    ("Refresh printers", "Drucker aktualisieren"),
    (
        "Remove this label from the import queue",
        "Dieses Etikett aus der Importwarteschlange entfernen",
    ),
    (
        "Requires a ZPL-compatible printer and raw CUPS queue.",
        "Erfordert einen ZPL-kompatiblen Drucker und eine rohe CUPS-Warteschlange.",
    ),
    ("Reset offset", "Versatz zurücksetzen"),
    (
        "Reset the crop to use the entire original page",
        "Zuschnitt zurücksetzen, um die gesamte Originalseite zu verwenden",
    ),
    ("Reset to entire page", "Auf ganze Seite zurücksetzen"),
    ("Right", "Rechts"),
    ("Rotate", "Drehen"),
    ("Rotation", "Drehung"),
    ("Save as…", "Speichern unter…"),
    ("Save changes?", "Änderungen speichern?"),
    ("Save design (Ctrl+S)", "Design speichern (Strg+S)"),
    (
        "Save your changes to keep them, or discard them to continue.",
        "Speichern Sie Ihre Änderungen, um sie zu behalten, oder verwerfen Sie sie, um fortzufahren.",
    ),
    (
        "Select an object on the canvas.",
        "Wählen Sie ein Objekt auf der Zeichenfläche.",
    ),
    ("Select label object", "Etikettenobjekt auswählen"),
    (
        "Select printer media in Print Center before printing.",
        "Wählen Sie vor dem Drucken das Druckermedium im Druckcenter.",
    ),
    (
        "Set the canvas dimensions to the selected printer stock. Object sizes and positions are preserved; Ctrl+Z undoes this change.",
        "Setzt die Maße der Zeichenfläche auf das gewählte Druckermedium. Objektgrößen und -positionen bleiben erhalten; Strg+Z macht die Änderung rückgängig.",
    ),
    (
        "Set up your printer and check your next print run.",
        "Richten Sie Ihren Drucker ein und prüfen Sie Ihren nächsten Druckauftrag.",
    ),
    (
        "Settings above are remembered for each printer. Use the offset if labels print slightly off-center.",
        "Die obigen Einstellungen werden pro Drucker gespeichert. Nutzen Sie den Versatz, wenn Etiketten leicht verschoben drucken.",
    ),
    (
        "Shape stroke width in millimeters",
        "Konturstärke der Form in Millimetern",
    ),
    (
        "Shift-click to select multiple objects. Use Ctrl+C/V, Ctrl+D, Delete, Ctrl+Z/Y.",
        "Mit Umschalt+Klick mehrere Objekte auswählen. Nutzen Sie Strg+C/V, Strg+D, Entf, Strg+Z/Y.",
    ),
    ("Show grid", "Raster anzeigen"),
    ("Show value below bars", "Wert unter den Balken anzeigen"),
    ("Snap to grid", "Am Raster ausrichten"),
    (
        "Text · ${date} and ${serial} are dynamic",
        "Text · ${date} und ${serial} sind dynamisch",
    ),
    ("Thermal label", "Thermoetikett"),
    (
        "These dimensions control the output and stock preview. Match them to the printer media size above.",
        "Diese Maße steuern die Ausgabe und die Medienvorschau. Passen Sie sie an das oben gewählte Druckermedium an.",
    ),
    (
        "This row cannot be previewed. Printing will report the problem.",
        "Diese Zeile kann nicht in der Vorschau angezeigt werden. Beim Drucken wird das Problem gemeldet.",
    ),
    ("Undo (Ctrl+Z)", "Rückgängig (Strg+Z)"),
    (
        "Update the output width and height to match the selected printer stock",
        "Ausgabebreite und -höhe an das gewählte Druckermedium anpassen",
    ),
    ("Vertical", "Vertikal"),
    (
        "Vertical print offset in millimeters",
        "Vertikaler Druckversatz in Millimetern",
    ),
    ("View", "Ansicht"),
    (
        "View the next label (Alt+Right)",
        "Nächstes Etikett anzeigen (Alt+Right)",
    ),
    (
        "View the original document and adjust the crop",
        "Originaldokument ansehen und Zuschnitt anpassen",
    ),
    (
        "View the previous label (Alt+Left)",
        "Vorheriges Etikett anzeigen (Alt+Left)",
    ),
    ("WORKSPACE", "ARBEITSBEREICH"),
    ("Watch a folder", "Ordner überwachen"),
    ("Watch for new labels", "Auf neue Etiketten achten"),
    (
        "Your label has unsaved changes.",
        "Ihr Etikett hat ungespeicherte Änderungen.",
    ),
    ("Your label workspace", "Ihr Etikettenarbeitsplatz"),
    ("Home", "Start"),
    ("Label Designer", "Etikettendesigner"),
    ("Import & Convert", "Importieren"),
    (
        "Open your workspace and label templates",
        "Arbeitsbereich und Etikettenvorlagen öffnen",
    ),
    (
        "Create and edit a custom label",
        "Ein eigenes Etikett erstellen und bearbeiten",
    ),
    (
        "Import, crop, and prepare existing labels",
        "Bestehende Etiketten importieren, zuschneiden und vorbereiten",
    ),
    (
        "Configure printer, stock, and queue settings",
        "Drucker, Medien und Warteschlange konfigurieren",
    ),
    ("Create a label", "Etikett erstellen"),
    ("Build something of your own", "Gestalten Sie etwas Eigenes"),
    (
        "Add text, shapes, barcodes and QR codes to a precisely sized canvas.",
        "Fügen Sie Text, Formen, Barcodes und QR-Codes zu einer exakt bemessenen Zeichenfläche hinzu.",
    ),
    ("Create new label", "Neues Etikett erstellen"),
    ("Import & convert", "Importieren und konvertieren"),
    (
        "Get existing labels ready to print",
        "Bestehende Etiketten druckfertig machen",
    ),
    (
        "Open PDFs or images, remove instructions, review crops, then print in order.",
        "PDFs oder Bilder öffnen, Anleitungen entfernen, Zuschnitte prüfen und der Reihe nach drucken.",
    ),
    ("Open existing labels", "Bestehende Etiketten öffnen"),
    ("Project & source code", "Projekt und Quellcode"),
    (
        "Documentation, releases, and source",
        "Dokumentation, Releases und Quellcode",
    ),
    ("Report an issue", "Problem melden"),
    (
        "Share a bug or suggest an improvement",
        "Einen Fehler melden oder eine Verbesserung vorschlagen",
    ),
    ("Created by", "Erstellt von"),
    (
        "Free to use, modify, and share",
        "Frei nutzbar, veränderbar und teilbar",
    ),
    ("Thermal labels", "Thermoetiketten"),
    ("Linux app", "Linux-App"),
    ("Open source", "Open Source"),
    ("Version copied", "Version kopiert"),
    (
        "Power tools for your desktop",
        "Leistungsstarke Werkzeuge für Ihren Desktop",
    ),
    ("Battery health for GNOME", "Akkugesundheit für GNOME"),
    ("Version", "Version"),
    ("Batch print from CSV", "Stapeldruck aus CSV"),
    ("Batch print from CSV…", "Stapeldruck aus CSV…"),
    ("Preparing", "Vorbereitung"),
    (
        "Wait for the current operation to finish.",
        "Warten Sie, bis der aktuelle Vorgang abgeschlossen ist.",
    ),
    ("Add a label", "Etikett hinzufügen"),
    (
        "Import a PDF or image to start preparing a print.",
        "Importieren Sie ein PDF oder Bild, um einen Druck vorzubereiten.",
    ),
    ("Adjust the layout", "Layout anpassen"),
    (
        "The label would be clipped. Use Fit within paper or change the output dimensions.",
        "Das Etikett würde abgeschnitten. Verwenden Sie „In Papier einpassen“ oder ändern Sie die Ausgabemaße.",
    ),
    ("Review the crop", "Zuschnitt prüfen"),
    (
        "Check the original crop and barcode edges, then confirm the review in Extract.",
        "Prüfen Sie den ursprünglichen Zuschnitt und die Barcode-Ränder und bestätigen Sie die Prüfung unter „Extrahieren“.",
    ),
    ("Preparing preview", "Vorschau wird erstellt"),
    (
        "The label preview is being generated.",
        "Die Etikettenvorschau wird erstellt.",
    ),
    ("Choose a printer", "Drucker wählen"),
    (
        "Select an available printer before printing. You can still export a prepared label.",
        "Wählen Sie vor dem Drucken einen verfügbaren Drucker. Ein vorbereitetes Etikett können Sie weiterhin exportieren.",
    ),
    ("Choose printer media", "Druckermedium wählen"),
    (
        "Select a supported media size that matches the loaded labels.",
        "Wählen Sie ein unterstütztes Medienformat, das zu den eingelegten Etiketten passt.",
    ),
    (
        "Label size differs from printer media",
        "Etikettengröße weicht vom Druckermedium ab",
    ),
    ("Ready to submit", "Bereit zum Senden"),
    ("Printer configured", "Drucker konfiguriert"),
    ("Verify the loaded stock", "Eingelegtes Medium prüfen"),
    (
        "The driver does not report dimensions for this media. Check that the loaded labels match the output size.",
        "Der Treiber meldet keine Maße für dieses Medium. Prüfen Sie, ob die eingelegten Etiketten der Ausgabegröße entsprechen.",
    ),
    (
        "Crop reviewed and output prepared. Check the loaded stock before printing.",
        "Zuschnitt geprüft und Ausgabe vorbereitet. Prüfen Sie vor dem Drucken das eingelegte Medium.",
    ),
    (
        "Media matches the output dimensions. Import or design a label to print.",
        "Das Medium entspricht den Ausgabemaßen. Importieren oder gestalten Sie ein Etikett zum Drucken.",
    ),
    ("Blank custom", "Leer (benutzerdefiniert)"),
    ("Shipping 4 × 6", "Versand 4 × 6"),
    ("Return address", "Absender"),
    ("Product barcode", "Produkt-Barcode"),
    ("Inventory", "Inventar"),
    ("QR code", "QR-Code"),
    ("Asset tracking", "Anlagenverfolgung"),
    ("Shelf", "Regal"),
    ("Price tag", "Preisschild"),
    ("Name label", "Namensschild"),
    ("Fragile", "Zerbrechlich"),
    ("Help", "Hilfe"),
    ("Help & Manual", "Hilfe & Handbuch"),
    ("Tray icon restored", "Infobereichsymbol wiederhergestellt"),
    (
        "Open the user manual in your browser (F1)",
        "Benutzerhandbuch im Browser öffnen (F1)",
    ),
    ("Save as template…", "Als Vorlage speichern…"),
    (
        "Keep this design in your Templates menu, named after the label",
        "Dieses Design im Vorlagenmenü behalten, benannt nach der Etikette",
    ),
    ("Smart guides", "Intelligente Hilfslinien"),
    (
        "Snap to other objects' edges and centers while dragging",
        "Beim Ziehen an Kanten und Mitten anderer Objekte einrasten",
    ),
    ("Show printable area", "Druckbereich anzeigen"),
    (
        "Shade the border your printer cannot print on",
        "Den Rand abdunkeln, den dein Drucker nicht bedrucken kann",
    ),
    (
        "These objects reach into the printer's unprintable border and may be cut off:",
        "Diese Objekte reichen in den nicht bedruckbaren Rand des Druckers und können abgeschnitten werden:",
    ),
    ("Your templates", "Deine Vorlagen"),
    ("Remove this template", "Diese Vorlage entfernen"),
    ("Grid size", "Rastergröße"),
    ("Grid size in millimeters", "Rastergröße in Millimetern"),
    ("Align", "Ausrichten"),
    ("Distribute evenly", "Gleichmäßig verteilen"),
    ("Center on label", "Auf der Etikette zentrieren"),
    ("Lock all", "Alle sperren"),
    ("Unlock all", "Alle entsperren"),
    ("Rotate 90°", "90° drehen"),
    (
        "Turn the object a quarter turn clockwise",
        "Das Objekt eine Vierteldrehung im Uhrzeigersinn drehen",
    ),
    ("Flip horizontally", "Horizontal spiegeln"),
    ("Flip vertically", "Vertikal spiegeln"),
    ("Lock position and size", "Position und Größe sperren"),
    ("Italic", "Kursiv"),
    ("Inverse", "Invertiert"),
    (
        "White letters on a black box",
        "Weiße Buchstaben auf schwarzem Kasten",
    ),
    ("Wrap long lines", "Lange Zeilen umbrechen"),
    (
        "Shrink to fit the box",
        "Verkleinern, bis es in den Rahmen passt",
    ),
    ("Line spacing", "Zeilenabstand"),
    ("Line spacing multiplier", "Zeilenabstand-Faktor"),
    ("Text size", "Textgröße"),
    (
        "Readable text size in points",
        "Größe des lesbaren Textes in Punkt",
    ),
    ("Text above the bars", "Text über den Balken"),
    ("QR error correction level", "QR-Fehlerkorrekturstufe"),
    ("Contrast", "Kontrast"),
    ("Image contrast in percent", "Bildkontrast in Prozent"),
    ("Cut-off", "Schwellenwert"),
    (
        "Pixels darker than this print black",
        "Dunklere Pixel werden schwarz gedruckt",
    ),
    ("Invert black and white", "Schwarz und Weiß vertauschen"),
    ("Reset image adjustments", "Bildanpassungen zurücksetzen"),
    (
        "Spacing of the guide grid. Snapping moves in steps of a fifth of it.",
        "Abstand des Hilfsrasters. Das Einrasten erfolgt in Schritten von einem Fünftel.",
    ),
    (
        "A locked object cannot be moved, resized, rotated, or deleted from the canvas",
        "Ein gesperrtes Objekt kann auf der Fläche nicht verschoben, skaliert, gedreht oder gelöscht werden",
    ),
    (
        "Break lines at spaces so they fit the box width",
        "Zeilen an Leerzeichen umbrechen, damit sie in die Rahmenbreite passen",
    ),
    (
        "Make the text smaller until all of it fits inside the box",
        "Den Text verkleinern, bis er vollständig in den Rahmen passt",
    ),
    (
        "More error correction survives more damage but makes a denser code",
        "Mehr Fehlerkorrektur übersteht mehr Schäden, ergibt aber einen dichteren Code",
    ),
    (
        "How the image is reduced to black and white",
        "Wie das Bild auf Schwarzweiß reduziert wird",
    ),
    ("Align left", "Linksbündig ausrichten"),
    ("Align center", "Zentriert ausrichten"),
    ("Align right", "Rechtsbündig ausrichten"),
    ("Align top", "Oben ausrichten"),
    ("Align middle", "In der Mitte ausrichten"),
    ("Align bottom", "Unten ausrichten"),
    ("Distribute horizontally", "Horizontal verteilen"),
    ("Distribute vertically", "Vertikal verteilen"),
    (
        "Center horizontally on label",
        "Horizontal auf der Etikette zentrieren",
    ),
    (
        "Center vertically on label",
        "Vertikal auf der Etikette zentrieren",
    ),
    (
        "Lock all selected objects",
        "Alle ausgewählten Objekte sperren",
    ),
    (
        "Unlock all selected objects",
        "Alle ausgewählten Objekte entsperren",
    ),
    (
        "Return the image mode, contrast, and cut-off to their defaults",
        "Bildmodus, Kontrast und Schwellenwert auf die Standardwerte zurücksetzen",
    ),
    ("Website", "Website"),
    ("Wi-Fi", "WLAN"),
    ("Contact card", "Visitenkarte"),
    ("Email", "E-Mail"),
    ("Phone number", "Telefonnummer"),
    ("Build the content", "Inhalt erstellen"),
    ("Kind of QR content", "Art des QR-Inhalts"),
    ("Web address", "Webadresse"),
    ("Network name", "Netzwerkname"),
    ("Password", "Passwort"),
    ("WPA / WPA2", "WPA / WPA2"),
    ("WEP", "WEP"),
    ("No password", "Kein Passwort"),
    ("Hidden network", "Verstecktes Netzwerk"),
    ("Name", "Name"),
    ("Organization", "Organisation"),
    ("Phone", "Telefon"),
    ("Email address", "E-Mail-Adresse"),
    ("Address", "Adresse"),
    ("Subject", "Betreff"),
    ("Message", "Nachricht"),
    ("Use this content", "Diesen Inhalt verwenden"),
    (
        "Replace the QR content with what you entered",
        "Den QR-Inhalt durch Ihre Eingaben ersetzen",
    ),
    (
        "Unsaved changes from your last session were found:",
        "Nicht gespeicherte Änderungen aus deiner letzten Sitzung wurden gefunden:",
    ),
    ("Restore", "Wiederherstellen"),
    (
        "Reopen the design from your last session as an unsaved copy",
        "Das Design deiner letzten Sitzung als ungespeicherte Kopie öffnen",
    ),
    (
        "Delete the recovered copy",
        "Die wiederhergestellte Kopie löschen",
    ),
    ("Template gallery", "Vorlagengalerie"),
    ("Search", "Suchen"),
    ("Search templates", "Vorlagen suchen"),
    (
        "Nothing saved yet. Use File → Save as template… to add one.",
        "Noch nichts gespeichert. Mit Datei → Als Vorlage speichern… kannst du eine hinzufügen.",
    ),
    ("Print on a sheet", "Auf einem Bogen drucken"),
    ("Paper", "Papier"),
    ("Resolution", "Auflösung"),
    ("Columns", "Spalten"),
    ("Rows", "Zeilen"),
    ("Page margin", "Seitenrand"),
    ("Page margin in millimeters", "Seitenrand in Millimetern"),
    ("Gap between labels", "Abstand zwischen Etiketten"),
    (
        "Gap between labels in millimeters",
        "Abstand zwischen Etiketten in Millimetern",
    ),
    ("How many labels", "Wie viele Etiketten"),
    ("Number of labels", "Anzahl der Etiketten"),
    ("Fit as many as possible", "So viele wie möglich"),
    (
        "Choose the most columns and rows of this label size that fit the page",
        "Die meisten Spalten und Zeilen dieser Etikettengröße wählen, die auf die Seite passen",
    ),
    (
        "The selected printer does not list this paper. You can still save a PDF.",
        "Der gewählte Drucker führt dieses Papier nicht auf. Du kannst trotzdem ein PDF speichern.",
    ),
    ("Save as PDF", "Als PDF speichern"),
    (
        "Save the sheet as a PDF file",
        "Den Bogen als PDF-Datei speichern",
    ),
    ("Print sheet", "Bogen drucken"),
    (
        "Send the sheet to the selected printer",
        "Den Bogen an den gewählten Drucker senden",
    ),
    ("labels per page", "Etiketten pro Seite"),
    ("page", "Seite"),
    ("pages", "Seiten"),
    ("Print a number range", "Nummernbereich drucken"),
    (
        "Put ${serial} in text, barcode, or QR content. Use ${serial:4} for numbers padded with zeros, like 0007. Add a prefix by typing it before the number, for example INV-${serial:4}.",
        "Setze ${serial} in Text, Barcode oder QR-Inhalt. Mit ${serial:4} werden Zahlen mit Nullen aufgefüllt, etwa 0007. Ein Präfix schreibst du vor die Zahl, zum Beispiel INV-${serial:4}.",
    ),
    ("First number", "Erste Nummer"),
    ("Step", "Schrittweite"),
    ("Step between numbers", "Schrittweite zwischen den Nummern"),
    ("Serials", "Seriennummern"),
    (
        "This design has no ${serial} yet, so every label would be the same.",
        "Dieses Design enthält noch kein ${serial}, daher wären alle Etiketten gleich.",
    ),
    ("Print labels:", "Etiketten drucken:"),
    (
        "Every label is checked first; nothing prints if one has a problem.",
        "Jede Etikette wird zuerst geprüft; bei einem Problem wird nichts gedruckt.",
    ),
    ("Template gallery…", "Vorlagengalerie…"),
    (
        "Browse the built-in templates and your own with previews",
        "Die integrierten und eigenen Vorlagen mit Vorschau durchsuchen",
    ),
    ("Print on a sheet…", "Auf einem Bogen drucken…"),
    ("Print a number range…", "Nummernbereich drucken…"),
    (
        "Print labels numbered by ${serial} with a start, step, and count",
        "Etiketten nummeriert mit ${serial} drucken, mit Start, Schritt und Anzahl",
    ),
    ("Address book", "Adressbuch"),
    (
        "Nothing saved yet. Select a text object and choose Save to address book.",
        "Noch nichts gespeichert. Wähle ein Textobjekt und dann Im Adressbuch speichern.",
    ),
    ("Remove this address", "Diese Adresse entfernen"),
    ("Symbols", "Symbole"),
    ("Your logos", "Deine Logos"),
    ("Remove this logo", "Dieses Logo entfernen"),
    ("Symbols and logos", "Symbole und Logos"),
    (
        "Auto-length (continuous roll)",
        "Automatische Länge (Endlosrolle)",
    ),
    (
        "Print only as long as the content needs, plus a bottom margin. Use it with continuous label rolls.",
        "Nur so lang drucken, wie der Inhalt braucht, plus unterer Rand. Für Endlos-Etikettenrollen.",
    ),
    ("Bottom margin", "Unterer Rand"),
    (
        "Bottom margin in millimeters",
        "Unterer Rand in Millimetern",
    ),
    ("Printed length", "Gedruckte Länge"),
    ("Save to address book", "Im Adressbuch speichern"),
    (
        "Keep this text, named after its first line, in your address book",
        "Diesen Text, benannt nach der ersten Zeile, im Adressbuch behalten",
    ),
    ("Font", "Schrift"),
    (
        "This font is not installed on this computer, so the built-in font is used.",
        "Diese Schrift ist auf diesem Computer nicht installiert, daher wird die integrierte verwendet.",
    ),
    (
        "Add a font from a .ttf or .otf file",
        "Eine Schrift aus einer .ttf- oder .otf-Datei hinzufügen",
    ),
    ("Save as logo", "Als Logo speichern"),
    (
        "Keep this image under Symbols and logos so you can add it to any label",
        "Dieses Bild unter Symbole und Logos behalten, um es zu jeder Etikette hinzuzufügen",
    ),
    ("Prints as", "Wird gedruckt als"),
    ("SSCC helper", "SSCC-Hilfe"),
    (
        "An SSCC numbers one pallet or parcel: extension digit, your GS1 company prefix, and a serial reference. The check digit is added for you.",
        "Ein SSCC nummeriert eine Palette oder ein Paket: Erweiterungsziffer, dein GS1-Firmenpräfix und eine Seriennummer. Die Prüfziffer wird für dich ergänzt.",
    ),
    ("Extension digit", "Erweiterungsziffer"),
    ("Company prefix", "Firmenpräfix"),
    ("Serial reference", "Seriennummer"),
    ("Use this SSCC", "Diesen SSCC verwenden"),
    (
        "Replace the barcode content with (00) and the SSCC",
        "Den Barcode-Inhalt durch (00) und den SSCC ersetzen",
    ),
    (
        "The logistics barcode for pallets and cartons. Type GS1 data such as (01)09506000134352(10)LOT7.",
        "Der Logistik-Barcode für Paletten und Kartons. GS1-Daten wie (01)09506000134352(10)LOT7 eingeben.",
    ),
    (
        "A small square symbol for GS1 data such as (01)09506000134352(10)LOT7. Put a batch or serial last.",
        "Ein kleines quadratisches Symbol für GS1-Daten wie (01)09506000134352(10)LOT7. Charge oder Serie ans Ende setzen.",
    ),
    ("Rotate 90° when printing", "Beim Drucken um 90° drehen"),
    (
        "For wide content: design the label turned on its side so the text reads straight, and it is rotated 90° when it prints, to fit the label stock as loaded.",
        "Für breite Inhalte: Das Etikett seitlich gedreht gestalten, damit der Text gerade zu lesen ist. Beim Drucken wird es um 90° gedreht, passend für die eingelegten Etiketten.",
    ),
    (
        "Rotated 90° counterclockwise to print on",
        "Zum Drucken um 90° gegen den Uhrzeigersinn gedreht auf",
    ),
    (
        "Rotated 90° clockwise to print on",
        "Zum Drucken um 90° im Uhrzeigersinn gedreht auf",
    ),
    ("Rotate the other way", "Andersherum drehen"),
    (
        "Which way the design faces on the printed label. Check “As printed” beside the canvas.",
        "In welche Richtung das Design auf dem gedruckten Etikett zeigt. Sieh dir „Gedruckt“ neben der Arbeitsfläche an.",
    ),
    (
        "Not available with rotated printing.",
        "Beim gedrehten Drucken nicht verfügbar.",
    ),
    ("Rotated printing.", "Gedreht drucken."),
    (
        "You are designing the label on its side. When it prints, it is rotated 90° counterclockwise to fit",
        "Du gestaltest das Etikett seitlich. Beim Drucken wird es um 90° gegen den Uhrzeigersinn gedreht, passend für",
    ),
    (
        "You are designing the label on its side. When it prints, it is rotated 90° clockwise to fit",
        "Du gestaltest das Etikett seitlich. Beim Drucken wird es um 90° im Uhrzeigersinn gedreht, passend für",
    ),
    (
        "See “As printed” in the corner.",
        "Siehe „Gedruckt“ in der Ecke.",
    ),
    ("As printed", "Gedruckt"),
    (
        "Rotated printing: Print rotates it 90° to fit",
        "Gedreht drucken: Drucken dreht es um 90°, passend für",
    ),
    ("Default (DejaVu Sans)", "Standard (DejaVu Sans)"),
    (
        "Built in · prints the same everywhere",
        "Integriert · druckt überall gleich",
    ),
    ("Your fonts", "Deine Schriften"),
    ("On this computer", "Auf diesem Computer"),
    ("Search fonts", "Schriften suchen"),
    ("No fonts match.", "Keine Schrift passt."),
    (
        "This font has no bold style. Choose a bold font instead.",
        "Diese Schrift hat keinen Fettschnitt. Wähle stattdessen eine fette Schrift.",
    ),
    (
        "Another computer needs this font installed to print the label the same way.",
        "Ein anderer Computer braucht diese Schrift, um das Etikett genauso zu drucken.",
    ),
    (
        "Condensed (Noto Sans Condensed)",
        "Schmal (Noto Sans Condensed)",
    ),
    (
        "Monospace (DejaVu Sans Mono)",
        "Dicktengleich (DejaVu Sans Mono)",
    ),
    ("Serif (DejaVu Serif)", "Serifen (DejaVu Serif)"),
    (
        "Headline (Noto Sans Display ExtraCondensed Black)",
        "Schlagzeile (Noto Sans Display ExtraCondensed Black)",
    ),
    ("Rounded (Varela Round)", "Abgerundet (Varela Round)"),
    ("Choose a font", "Schrift wählen"),
    // Label footer
    ("Label footer", "Etikettenfußzeile"),
    (
        "The footer QR code was left out because there is not enough blank space at the bottom of this label for it. The message still prints.",
        "Der QR-Code der Fußzeile wurde weggelassen, weil unten auf diesem Etikett nicht genug freier Platz dafür ist. Die Nachricht wird trotzdem gedruckt.",
    ),
    ("Print a footer message", "Fußzeilennachricht drucken"),
    ("Footer message", "Fußzeilennachricht"),
    ("QR code link or text", "Link oder Text des QR-Codes"),
    ("Footer QR code content", "Inhalt des Fußzeilen-QR-Codes"),
    (
        "Enter a message to print the footer. The QR code prints only with a message.",
        "Geben Sie eine Nachricht ein, um die Fußzeile zu drucken. Der QR-Code wird nur mit einer Nachricht gedruckt.",
    ),
    (
        "Scan a test print with your phone before printing a batch.",
        "Scannen Sie einen Testdruck mit Ihrem Telefon, bevor Sie einen Stapel drucken.",
    ),
    ("Reset footer", "Fußzeile zurücksetzen"),
    (
        "The QR text is too long to encode. Shorten it.",
        "Der QR-Text ist zu lang zum Kodieren. Kürzen Sie ihn.",
    ),
    (
        "The footer QR code was left out because it would be large on this short label. The message still prints.",
        "Der QR-Code der Fußzeile wurde weggelassen, weil er auf diesem kurzen Etikett zu groß wäre. Die Nachricht wird trotzdem gedruckt.",
    ),
    // 0.15.0: designer footer, overlap warning, symbols
    (
        "Add a QR code before the message",
        "QR-Code vor der Nachricht hinzufügen",
    ),
    (
        "Turn the footer off and restore the default message, QR code link, and alignment",
        "Fußzeile ausschalten und Standardnachricht, QR-Code-Link und Ausrichtung wiederherstellen",
    ),
    (
        "The footer was left out: part of the design is in the blank space it needs at the bottom of the label. Move objects up, choose another alignment, or turn the footer off. The label size is unchanged.",
        "Die Fußzeile wurde weggelassen: Ein Teil des Designs liegt im freien Bereich, den sie unten auf dem Etikett braucht. Verschieben Sie Objekte nach oben, wählen Sie eine andere Ausrichtung oder schalten Sie die Fußzeile aus. Die Etikettengröße bleibt gleich.",
    ),
    (
        "The footer was left out: the label is too small for it inside the printable area.",
        "Die Fußzeile wurde weggelassen: Das Etikett ist im druckbaren Bereich zu klein dafür.",
    ),
    (
        "Adds a small message, and if you like a QR code, along the bottom of labels you design here when you print them. Imported shipping labels never get a footer.",
        "Fügt beim Drucken eine kleine Nachricht und auf Wunsch einen QR-Code unten auf den hier gestalteten Etiketten hinzu. Importierte Versandetiketten erhalten nie eine Fußzeile.",
    ),
    (
        "Choose where the whole footer sits along the bottom edge. Pick a side where your design leaves blank space.",
        "Wählen Sie, wo die ganze Fußzeile am unteren Rand sitzt. Nehmen Sie eine Seite, auf der Ihr Design Platz frei lässt.",
    ),
    (
        "The label size and printable area never change. If that spot is not blank or is outside the printable area, the QR code, or the whole footer, is left out and a note says why. Exports do not include the footer.",
        "Etikettengröße und druckbarer Bereich ändern sich nie. Ist diese Stelle nicht frei oder liegt sie außerhalb des druckbaren Bereichs, wird der QR-Code oder die ganze Fußzeile weggelassen, und ein Hinweis nennt den Grund. Exporte enthalten die Fußzeile nicht.",
    ),
    (
        "Put the whole footer, QR code included, at the bottom-left",
        "Die ganze Fußzeile samt QR-Code unten links platzieren",
    ),
    (
        "Put the whole footer, QR code included, at the bottom center",
        "Die ganze Fußzeile samt QR-Code unten mittig platzieren",
    ),
    (
        "Put the whole footer, QR code included, at the bottom-right",
        "Die ganze Fußzeile samt QR-Code unten rechts platzieren",
    ),
    (
        "The footer fits: the canvas shows it where it will print.",
        "Die Fußzeile passt: Die Arbeitsfläche zeigt sie dort, wo sie gedruckt wird.",
    ),
    ("Footer", "Fußzeile"),
    (
        "Show the label footer settings in the properties panel",
        "Einstellungen der Etikettenfußzeile im Eigenschaftenbereich anzeigen",
    ),
    (
        "Print is off while a barcode or QR code overlaps another object. Move or resize them so they no longer touch, and Print turns back on.",
        "Drucken ist aus, solange ein Barcode oder QR-Code ein anderes Objekt überlappt. Verschieben Sie sie oder ändern Sie die Größe, bis sie sich nicht mehr berühren, dann ist Drucken wieder verfügbar.",
    ),
    (
        "Overlap: these codes touch another object, so Print is off until they are apart. Move or resize them:",
        "Überlappung: Diese Codes berühren ein anderes Objekt, daher ist Drucken aus, bis sie getrennt sind. Verschieben oder Größe ändern:",
    ),
    ("This side up", "Diese Seite oben"),
    ("Keep dry", "Vor Nässe schützen"),
    ("Recycling", "Recycling"),
    ("Hazard", "Gefahr"),
    ("Do not stack", "Nicht stapeln"),
    ("Keep cool", "Kühl lagern"),
    ("Keep out of sunlight", "Vor Sonnenlicht schützen"),
    ("Package", "Paket"),
    // Interface reorganization
    (
        "Check every barcode and QR code for content, size, clear space, overlap, and clipping",
        "Jeden Barcode und QR-Code auf Inhalt, Größe, Ruhezone, Überlappung und Abschneiden prüfen",
    ),
    (
        "Choose a printer on the Printer tab first. Stock profiles belong to a printer.",
        "Wählen Sie zuerst auf dem Tab Drucker einen Drucker. Etikettenprofile gehören zu einem Drucker.",
    ),
    ("Compact layout", "Kompaktes Layout"),
    ("Hide other notes", "Weitere Hinweise ausblenden"),
    ("Label footer…", "Etikettenfußzeile…"),
    ("Label footer (on)…", "Etikettenfußzeile (an)…"),
    (
        "Labels you print are listed here, so you can open or reprint them.",
        "Gedruckte Etiketten erscheinen hier, damit Sie sie öffnen oder erneut drucken können.",
    ),
    ("More notes", "Weitere Hinweise"),
    (
        "Tighter spacing, so more fits on a small screen",
        "Engere Abstände, damit auf kleinen Bildschirmen mehr Platz ist",
    ),
    ("Zoom to 100%", "Auf 100 % zoomen"),
    ("Printer", "Drucker"),
    ("Automation", "Automatisierung"),
    (
        "Place copies of this label on one page of any paper size your printer supports, for an office printer or a sheet of sticky labels. ${serial} counts up from one label to the next.",
        "Platziert Kopien dieses Etiketts auf einer Seite in jedem Papierformat, das dein Drucker unterstützt, für einen Bürodrucker oder einen Bogen Klebeetiketten. ${serial} zählt von einem Etikett zum nächsten hoch.",
    ),
    ("Printer paper size", "Papierformat des Druckers"),
    ("Custom size", "Eigenes Format"),
    (
        "Place several labels on one page of any paper size",
        "Mehrere Etiketten auf einer Seite in beliebigem Papierformat platzieren",
    ),
    (
        "Choose the paper or labels loaded in the printer. Custom uses the output dimensions.",
        "Wähle das Papier oder die Etiketten, die im Drucker liegen. Eigene Maße verwendet die Ausgabegröße.",
    ),
    (
        "Choose a printer and paper size, check alignment, and save it all as a stock profile you can load later.",
        "Wähle Drucker und Papierformat, prüfe die Ausrichtung und speichere alles als Materialprofil, das du später laden kannst.",
    ),
    ("Use the paper size", "Papierformat übernehmen"),
    ("Choose printer", "Drucker wählen"),
    ("Looking for printers…", "Drucker werden gesucht…"),
    (
        "No printers found. Add a printer in your system settings, then refresh.",
        "Keine Drucker gefunden. Füge in den Systemeinstellungen einen Drucker hinzu und aktualisiere dann.",
    ),
    ("Choose a printer first.", "Wähle zuerst einen Drucker."),
    ("Paper or label size", "Papier- oder Etikettenformat"),
    ("Custom dimensions", "Eigene Maße"),
    (
        "This printer does not list its paper sizes. Configure them in its driver.",
        "Dieser Drucker nennt keine Papierformate. Richte sie in seinem Treiber ein.",
    ),
    ("Saves:", "Speichert:"),
    (
        "For example: 4 × 6 shipping labels",
        "Zum Beispiel: 4 × 6 Versandetiketten",
    ),
    ("Replace stock profile", "Materialprofil ersetzen"),
    ("Stock profile saved", "Materialprofil gespeichert"),
    (
        "A profile with this name already exists for this printer. Saving replaces it.",
        "Für diesen Drucker gibt es schon ein Profil mit diesem Namen. Speichern ersetzt es.",
    ),
    (
        "You have reached the limit of saved stock profiles. Delete one to add another.",
        "Du hast die Höchstzahl gespeicherter Materialprofile erreicht. Lösche eines, um ein weiteres hinzuzufügen.",
    ),
    (
        "A stock profile remembers a printer with its paper size, resolution, and alignment. Load one to switch to all of it at once.",
        "Ein Materialprofil merkt sich einen Drucker mit Papierformat, Auflösung und Ausrichtung. Lade eines, um alles auf einmal umzustellen.",
    ),
    (
        "No saved stock yet. Set up the printer and paper, then save them below.",
        "Noch kein Material gespeichert. Richte Drucker und Papier ein und speichere sie unten.",
    ),
    ("Load", "Laden"),
    (
        "This printer is not connected",
        "Dieser Drucker ist nicht verbunden",
    ),
    ("Delete stock profile?", "Materialprofil löschen?"),
    ("Stock profile deleted", "Materialprofil gelöscht"),
    (
        "Choose the printer you will print on.",
        "Wähle den Drucker, auf dem du drucken wirst.",
    ),
    (
        "Choose the size loaded in the printer. Label printers list their label sizes; office printers list every paper size they support, such as Letter, Legal, or A4.",
        "Wähle das eingelegte Format. Etikettendrucker zeigen ihre Etikettenformate; Bürodrucker jedes unterstützte Papierformat, etwa Letter, Legal oder A4.",
    ),
    (
        "Optional. Print one test page and compare its 1 mm border to the edges. If it is off, move it with the offsets.",
        "Optional. Drucke eine Testseite und vergleiche ihren 1-mm-Rand mit den Kanten. Liegt er daneben, verschiebe ihn mit den Versätzen.",
    ),
    (
        "Name this setup so you can switch back to it in one click from Label stock.",
        "Gib dieser Einrichtung einen Namen, um unter Etikettenmaterial mit einem Klick zu ihr zurückzukehren.",
    ),
    (
        "It is now the active profile. Switch profiles any time from Active profile at the top of Print Center.",
        "Es ist jetzt das aktive Profil. Wechsle Profile jederzeit über Aktives Profil oben im Druckcenter.",
    ),
    ("Finish", "Fertig"),
    ("Set up another", "Weiteres einrichten"),
    ("Active profile:", "Aktives Profil:"),
    ("Stock profile updated", "Materialprofil aktualisiert"),
    ("None", "Keines"),
    ("Changed since it was loaded", "Seit dem Laden geändert"),
    ("Update profile", "Profil aktualisieren"),
    ("Active", "Aktiv"),
    (
        "Save the current printer, paper, resolution, and alignment into this profile",
        "Aktuellen Drucker, Papier, Auflösung und Ausrichtung in diesem Profil speichern",
    ),
    (
        "Changed since it was loaded. Update profile saves the changes.",
        "Seit dem Laden geändert. Profil aktualisieren speichert die Änderungen.",
    ),
    ("Saved and active:", "Gespeichert und aktiv:"),
    (
        "Save the current setup as a new profile",
        "Aktuelle Einrichtung als neues Profil speichern",
    ),
    ("Unsaved", "Nicht gespeichert"),
    ("rotated printing", "gedrehter Druck"),
    ("{count} objects", "{count} Objekte"),
    ("1 copy", "1 Kopie"),
    ("{count} copies", "{count} Kopien"),
    ("Printer media: {media}", "Druckermedium: {media}"),
    ("Not selected", "Nicht ausgewählt"),
    (
        "Custom — matches output",
        "Benutzerdefiniert — entspricht der Ausgabe",
    ),
    (
        "{count} imported labels queued",
        "{count} importierte Etiketten in der Warteschlange",
    ),
    (
        "Print queue · {count} labels",
        "Druckwarteschlange · {count} Etiketten",
    ),
    ("Label {n} of {total}", "Etikett {n} von {total}"),
    ("Queued labels", "Etiketten in der Warteschlange"),
    (
        "Print all {count} labels in order",
        "Alle {count} Etiketten der Reihe nach drucken",
    ),
    ("Recently opened", "Zuletzt geöffnet"),
    ("of {total}", "von {total}"),
    (
        "Advanced layout & resolution",
        "Erweitertes Layout und Auflösung",
    ),
    (
        "Demo workspace · All sample labels are fictional",
        "Demo-Arbeitsbereich · Alle Beispieletiketten sind fiktiv",
    ),
    ("Automatic", "Automatisch"),
    ("Manual", "Manuell"),
    ("Full page", "Ganze Seite"),
    ("Preview", "Vorschau"),
    ("01  EXTRACT", "01  EXTRAHIEREN"),
    ("02  PAPER & LAYOUT", "02  PAPIER & LAYOUT"),
    ("03  PRINT & EXPORT", "03  DRUCKEN & EXPORTIEREN"),
    (
        "Automatically detect the label region",
        "Etikettenbereich automatisch erkennen",
    ),
    (
        "Manually adjust the crop on the original document",
        "Zuschnitt im Originaldokument manuell anpassen",
    ),
    (
        "Use the entire page without cropping",
        "Ganze Seite ohne Zuschnitt verwenden",
    ),
    (
        "Paper boundary · Final output uses the full-resolution render",
        "Papiergrenze · Die endgültige Ausgabe nutzt das Rendering in voller Auflösung",
    ),
    (
        "Original document · Green boundary is the crop · Crop before rotating",
        "Originaldokument · Der grüne Rahmen ist der Zuschnitt · Vor dem Drehen zuschneiden",
    ),
    (
        "Likely thermal label printer",
        "Wahrscheinlich ein Thermoetikettendrucker",
    ),
    (
        "Likely label printer · check media",
        "Wahrscheinlich ein Etikettendrucker · Medium prüfen",
    ),
    (
        "Supports label-sized media · verify printer type",
        "Unterstützt Etikettenformate · Druckertyp prüfen",
    ),
    (
        "Printer type unknown · check its driver and paper",
        "Druckertyp unbekannt · Treiber und Papier prüfen",
    ),
    ("Margins & image enhancement", "Ränder und Bildverbesserung"),
    (
        "Print through the printer driver",
        "Über den Druckertreiber drucken",
    ),
    (
        "Recommended. The printer's driver handles the label size and printing.",
        "Empfohlen. Der Druckertreiber übernimmt Etikettengröße und Druck.",
    ),
    ("Print using raw TSPL", "Mit Roh-TSPL drucken"),
    (
        "Raw TSPL: verify printer compatibility and loaded label size.",
        "Roh-TSPL: Druckerkompatibilität und eingelegte Etikettengröße prüfen.",
    ),
    (
        "For TSPL printers such as many 4 × 6 shipping printers. Requires a raw CUPS queue. The printer keeps its own gap calibration.",
        "Für TSPL-Drucker wie viele 4 × 6-Versanddrucker. Erfordert eine rohe CUPS-Warteschlange. Der Drucker behält seine eigene Lückenkalibrierung.",
    ),
    (
        "For TSPL printers such as many 4 × 6 shipping printers. Requires a printer that accepts RAW jobs. The printer keeps its own gap calibration.",
        "Für TSPL-Drucker wie viele 4 × 6-Versanddrucker. Erfordert einen Drucker, der RAW-Aufträge annimmt. Der Drucker behält seine eigene Lückenkalibrierung.",
    ),
];
const FR: &[(&str, &str)] = &[
    ("No matches", "Aucun résultat"),
    ("Open ThermalPrint", "Ouvrir ThermalPrint"),
    ("Check barcodes", "Vérifier les codes"),
    ("Background printing", "Impression en arrière-plan"),
    (
        "Keep running when the window is closed",
        "Continuer après la fermeture de la fenêtre",
    ),
    (
        "The tray menu opens ThermalPrint, pauses automatic printing, or quits. Your computer must be awake.",
        "Le menu de notification permet d’ouvrir ThermalPrint, de suspendre l’impression automatique ou de quitter. L’ordinateur doit rester éveillé.",
    ),
    ("Start at login", "Démarrer à la connexion"),
    (
        "Automatic printing checks file contents to prevent repeat prints, even after a file is renamed.",
        "L’impression automatique vérifie le contenu pour éviter les doublons, même après un changement de nom.",
    ),
    (
        "Clear duplicate protection…",
        "Réinitialiser la protection contre les doublons…",
    ),
    (
        "Clear duplicate protection?",
        "Réinitialiser la protection contre les doublons ?",
    ),
    (
        "Previously printed files can print automatically again if they arrive in the watch folder.",
        "Les fichiers déjà imprimés pourront être réimprimés automatiquement s’ils arrivent dans le dossier surveillé.",
    ),
    (
        "Clear duplicate protection",
        "Réinitialiser la protection contre les doublons",
    ),
    (
        "Printer setup assistant",
        "Assistant de configuration de l’imprimante",
    ),
    (
        "Set up printer and label stock",
        "Configurer l’imprimante et les étiquettes",
    ),
    ("Next", "Suivant"),
    ("Print one test label", "Imprimer une étiquette de test"),
    ("Close assistant", "Fermer l’assistant"),
    ("Saved label stock", "Supports d’étiquettes enregistrés"),
    ("Stock profile name", "Nom du profil de support"),
    ("Save stock profile", "Enregistrer le profil de support"),
    ("Reprint…", "Réimprimer…"),
    (
        "Review and reprint with the recorded settings",
        "Vérifier et réimprimer avec les paramètres enregistrés",
    ),
    ("Reprint label?", "Réimprimer l’étiquette ?"),
    (
        "This sends another print job using the recorded settings.",
        "Cela envoie une nouvelle tâche avec les paramètres enregistrés.",
    ),
    ("Reprint", "Réimprimer"),
    ("Cable flag", "Étiquette de câble"),
    (
        "Matching cable IDs on both sides of a fold",
        "Même identifiant de câble des deux côtés du pli",
    ),
    ("Price and barcode", "Prix et code-barres"),
    (
        "Product, price, and scannable stock code",
        "Produit, prix et code article lisible",
    ),
    ("Shelf barcode", "Code-barres de rayon"),
    (
        "Shelf location with a stock barcode",
        "Emplacement de rayon avec code article",
    ),
    ("Small product QR", "Petit QR code produit"),
    (
        "Compact product details and QR code",
        "Informations produit compactes et QR code",
    ),
    ("Printing", "Impression de"),
    (
        "Print labels automatically?",
        "Imprimer les étiquettes automatiquement ?",
    ),
    (
        "Every label that lands in the watch folder will print by itself, without you pressing Print. A folder used only for labels is safest.",
        "Chaque étiquette qui arrive dans le dossier surveillé s'imprimera toute seule, sans que vous appuyiez sur Imprimer. Un dossier réservé aux étiquettes est le plus sûr.",
    ),
    (
        "This looks like your Downloads folder: an invoice or any other PDF saved there could print too. A separate folder just for labels is safer.",
        "Cela ressemble à votre dossier Téléchargements : une facture ou tout autre PDF enregistré là pourrait aussi s'imprimer. Un dossier séparé réservé aux étiquettes est plus sûr.",
    ),
    (
        "Only single-page labels whose crop needs no review print. Each one waits 5 seconds with a Cancel button, and Pause stops everything.",
        "Seules les étiquettes d'une page dont le recadrage ne demande aucune vérification s'impriment. Chacune attend 5 secondes avec un bouton Annuler, et Pause arrête tout.",
    ),
    ("Turn on", "Activer"),
    ("auto-print paused", "impression auto en pause"),
    ("auto-print on", "impression auto activée"),
    ("Resume auto-print", "Reprendre l'impression auto"),
    ("Pause auto-print", "Mettre l'impression auto en pause"),
    (
        "Stop or restart printing labels by themselves. Waiting labels stay in the queue.",
        "Arrête ou relance l'impression automatique des étiquettes. Les étiquettes en attente restent dans la file.",
    ),
    (
        "Print labels automatically",
        "Imprimer les étiquettes automatiquement",
    ),
    (
        "Print labels that land in this folder by themselves. Only single-page labels whose crop needs no review print; the rest wait in the queue. Off until you turn it on.",
        "Imprime toutes seules les étiquettes qui arrivent dans ce dossier. Seules les étiquettes d'une page dont le recadrage ne demande aucune vérification s'impriment ; les autres attendent dans la file. Désactivé tant que vous ne l'activez pas.",
    ),
    ("Only files named", "Seulement les fichiers nommés"),
    (
        "Leave empty to print every label. Use * for any letters and ? for one letter; separate several names with commas.",
        "Laissez vide pour imprimer toutes les étiquettes. Utilisez * pour n'importe quelles lettres et ? pour une lettre ; séparez plusieurs noms par des virgules.",
    ),
    (
        "File names that print automatically",
        "Noms de fichiers imprimés automatiquement",
    ),
    ("Most per minute", "Maximum par minute"),
    (
        "A burst of files cannot print more labels than this in one minute; the rest wait.",
        "Une rafale de fichiers ne peut pas imprimer plus d'étiquettes que ce nombre en une minute ; les autres attendent.",
    ),
    (
        "Most labels printed automatically per minute",
        "Nombre maximal d'étiquettes imprimées automatiquement par minute",
    ),
    ("Safe margin", "Marge de sécurité"),
    (
        "Keep content this far from the label's edge. It is shaded on the canvas and objects inside it are flagged, even when the printer's driver reports no unprintable border. It is remembered for each printer.",
        "Gardez le contenu à cette distance du bord de l'étiquette. La zone est grisée sur le canevas et les objets qui s'y trouvent sont signalés, même si le pilote de l'imprimante ne signale aucun bord non imprimable. Elle est mémorisée pour chaque imprimante.",
    ),
    (
        "Safe margin in millimeters",
        "Marge de sécurité en millimètres",
    ),
    (
        "These objects reach into the unprintable border or your safe margin and may be cut off:",
        "Ces objets atteignent le bord non imprimable ou votre marge de sécurité et peuvent être coupés :",
    ),
    (
        "Choose a printer in Print Center to see its unprintable border, or set a safe margin in the View menu.",
        "Choisissez une imprimante dans le Centre d'impression pour voir son bord non imprimable, ou définissez une marge de sécurité dans le menu Affichage.",
    ),
    (
        "Select printer media in Print Center to see its unprintable border, or set a safe margin in the View menu.",
        "Choisissez le support dans le Centre d'impression pour voir son bord non imprimable, ou définissez une marge de sécurité dans le menu Affichage.",
    ),
    (
        "Choose media that matches this label's size to see the printer's unprintable border, or set a safe margin in the View menu.",
        "Choisissez un support qui correspond à la taille de cette étiquette pour voir le bord non imprimable de l'imprimante, ou définissez une marge de sécurité dans le menu Affichage.",
    ),
    (
        "The printer's driver reports no unprintable border for this media. Set a safe margin in the View menu to keep content away from the edge.",
        "Le pilote de l'imprimante ne signale aucun bord non imprimable pour ce support. Définissez une marge de sécurité dans le menu Affichage pour garder le contenu loin du bord.",
    ),
    (
        "Keeps designs this far from the label's edge. Use it when your printer cuts off the edges but its driver reports no unprintable border. It is remembered for each printer.",
        "Garde les modèles à cette distance du bord de l'étiquette. À utiliser quand votre imprimante coupe les bords alors que son pilote ne signale aucun bord non imprimable. Mémorisée pour chaque imprimante.",
    ),
    ("Logo in the center", "Logo au centre"),
    (
        "A logo hides part of the code. ThermalPrint keeps it small enough, and uses the highest error correction, so phones can still scan it.",
        "Un logo masque une partie du code. ThermalPrint le garde assez petit et utilise la correction d'erreurs maximale, pour que les téléphones puissent toujours le scanner.",
    ),
    ("Add logo…", "Ajouter un logo…"),
    ("Change logo…", "Changer le logo…"),
    ("Remove logo", "Retirer le logo"),
    ("Choose a saved logo", "Choisir un logo enregistré"),
    ("Logo size", "Taille du logo"),
    (
        "The share of the code's width that is cleared for the logo",
        "La part de la largeur du code libérée pour le logo",
    ),
    (
        "Largest size that scans reliably:",
        "Taille maximale qui se scanne de façon fiable :",
    ),
    (
        "Error correction is set to High while a logo is used.",
        "La correction d'erreurs est réglée sur Élevée tant qu'un logo est utilisé.",
    ),
    (
        "The code is small for a logo. Make it at least this wide:",
        "Le code est petit pour un logo. Donnez-lui au moins cette largeur :",
    ),
    ("No printer selected", "Aucune imprimante sélectionnée"),
    (
        "No recent documents yet. Labels you open will appear here.",
        "Aucun document récent pour l'instant. Les étiquettes que vous ouvrez apparaîtront ici.",
    ),
    ("Reviewed", "Vérifiée"),
    (
        "Print this label (Ctrl+P)",
        "Imprimer cette étiquette (Ctrl+P)",
    ),
    ("Add a label first", "Ajoutez d'abord une étiquette"),
    (
        "The preview is still being prepared",
        "L'aperçu est encore en préparation",
    ),
    (
        "The label would be clipped. Adjust the layout",
        "L'étiquette serait coupée. Ajustez la mise en page",
    ),
    (
        "Tick Reviewed once you have checked the crop",
        "Cochez Vérifiée une fois le recadrage contrôlé",
    ),
    (
        "Wait for the current operation to finish",
        "Attendez la fin de l'opération en cours",
    ),
    ("Undo", "Annuler"),
    ("Dismiss", "Fermer"),
    ("Watching", "Surveillance de"),
    ("Hide this", "Masquer"),
    ("Turn watching back on", "Réactiver la surveillance"),
    ("Recent messages", "Messages récents"),
    ("Show recent messages", "Afficher les messages récents"),
    ("Add from Processed…", "Ajouter depuis Processed…"),
    (
        "Keep going if a label fails",
        "Continuer si une étiquette échoue",
    ),
    ("Open Import & Convert", "Ouvrir Importer et convertir"),
    ("Messages", "Messages"),
    ("Will print", "Impression prévue"),
    (
        "Labels added from now on count as reviewed, so Print all does not wait for you to check each crop. Turn this on only for labels whose layout you trust.",
        "Les étiquettes ajoutées à partir de maintenant comptent comme vérifiées, donc Tout imprimer n'attend pas la vérification de chaque recadrage. À activer uniquement pour des étiquettes dont vous connaissez la mise en page.",
    ),
    (
        "Choose labels from the Processed folder to print them again",
        "Choisissez des étiquettes du dossier Processed pour les réimprimer",
    ),
    (
        "Print all skips a label that fails and continues with the rest. Failed labels stay in the queue.",
        "Tout imprimer ignore une étiquette en échec et continue avec les autres. Les étiquettes en échec restent dans la file.",
    ),
    ("Nothing yet", "Rien pour l'instant"),
    ("Current printer", "Imprimante actuelle"),
    (
        "Printer for watched labels",
        "Imprimante pour les étiquettes surveillées",
    ),
    (
        "Skip review for automatic crops",
        "Ignorer la vérification des recadrages automatiques",
    ),
    ("Get ready to print", "Préparez-vous à imprimer"),
    (
        "Choose your label printer in Import & Convert",
        "Choisissez votre imprimante d'étiquettes dans Importer et convertir",
    ),
    (
        "Add your label printer in your system settings, then reopen the app",
        "Ajoutez votre imprimante d'étiquettes dans les réglages du système, puis rouvrez l'application",
    ),
    (
        "Choose the label size your printer has loaded",
        "Choisissez la taille d'étiquette chargée dans votre imprimante",
    ),
    (
        "Optional: watch a folder such as Downloads so new labels appear by themselves",
        "Facultatif : surveillez un dossier comme Téléchargements pour que les nouvelles étiquettes apparaissent d'elles-mêmes",
    ),
    ("Restore your last queue?", "Restaurer la dernière file ?"),
    (
        "Move printed labels to a Processed folder",
        "Déplacer les étiquettes imprimées vers un dossier Processed",
    ),
    (
        "After printing, labels from the watch folder are removed from the queue and moved into a Processed folder inside it. Labels added from elsewhere are not moved.",
        "Après l'impression, les étiquettes du dossier surveillé sont retirées de la file et déplacées dans un dossier Processed à l'intérieur. Les étiquettes ajoutées depuis ailleurs ne sont pas déplacées.",
    ),
    ("About ThermalPrint", "À propos de ThermalPrint"),
    ("Add labels…", "Ajouter des étiquettes…"),
    ("Advanced driver resolution", "Résolution avancée du pilote"),
    ("Copies", "Copies"),
    ("Height", "Hauteur"),
    ("Width", "Largeur"),
    ("W", "L"),
    ("H", "H"),
    (
        "Import PDF or image labels (Ctrl+O)",
        "Importer des étiquettes PDF ou image (Ctrl+O)",
    ),
    ("Printing service: CUPS", "Service d'impression : CUPS"),
    (
        "Refresh available CUPS printers",
        "Actualiser les imprimantes CUPS disponibles",
    ),
    ("Rendering DPI", "DPI de rendu"),
    ("Save", "Enregistrer"),
    ("Serial variable", "Variable de série"),
    (
        "Swap label width and height",
        "Intervertir la largeur et la hauteur de l'étiquette",
    ),
    ("Swap portrait / landscape", "Basculer portrait / paysage"),
    ("Templates", "Modèles"),
    ("Zoom in", "Zoom avant"),
    ("Zoom out", "Zoom arrière"),
    (
        "A successful submission means CUPS accepted the job; check physical output and barcode readability.",
        "Un envoi réussi signifie que CUPS a accepté le travail ; vérifiez l'impression et la lisibilité du code-barres.",
    ),
    ("Actual size", "Taille réelle"),
    (
        "Add new PDF and image labels automatically when they appear in a folder, such as Downloads. Files already there when you turn this on are ignored; labels saved while ThermalPrint is closed are added when it opens.",
        "Ajouter automatiquement les nouvelles étiquettes PDF et image dès qu'elles apparaissent dans un dossier, comme Téléchargements. Les fichiers déjà présents à l'activation sont ignorés ; les étiquettes enregistrées quand ThermalPrint est fermé sont ajoutées à son ouverture.",
    ),
    ("Alignment", "Alignement"),
    ("Back", "Retour"),
    ("Backward", "Vers l'arrière"),
    ("Barcode or QR code content", "Contenu du code-barres ou QR"),
    (
        "Black-and-white threshold (advanced)",
        "Seuil noir et blanc (avancé)",
    ),
    ("Bold", "Gras"),
    ("Cancel", "Annuler"),
    ("Center", "Centre"),
    ("Check CUPS job status", "Vérifier l'état des travaux CUPS"),
    (
        "Check jobs submitted to the selected CUPS printer",
        "Vérifie les travaux envoyés à l'imprimante CUPS sélectionnée",
    ),
    (
        "Check jobs submitted to the selected printer",
        "Vérifie les travaux envoyés à l'imprimante sélectionnée",
    ),
    (
        "Check the original crop, then confirm it in the left panel.",
        "Vérifiez le recadrage d'origine, puis confirmez-le dans le panneau de gauche.",
    ),
    ("Choose CSV file…", "Choisir un fichier CSV…"),
    (
        "Choose a printer in Print Center first.",
        "Choisissez d'abord une imprimante dans le Centre d'impression.",
    ),
    (
        "Choose a printer in Print Center to print this design. Export is available without a printer.",
        "Choisissez une imprimante dans le Centre d'impression pour imprimer ce modèle. L'export reste possible sans imprimante.",
    ),
    ("Choose folder…", "Choisir un dossier…"),
    ("Clear", "Retirer"),
    ("Clear history", "Effacer l'historique"),
    ("Close", "Fermer"),
    (
        "Copy version information",
        "Copier les informations de version",
    ),
    ("Credits and license", "Crédits et licence"),
    ("Custom scale", "Échelle personnalisée"),
    (
        "Default processing preserves tones. Thresholding can damage fine barcode detail.",
        "Le traitement par défaut préserve les tons. Le seuillage peut abîmer les détails fins du code-barres.",
    ),
    ("Delete", "Supprimer"),
    (
        "Delete all selected objects",
        "Supprimer tous les objets sélectionnés",
    ),
    ("Delete selected", "Supprimer la sélection"),
    (
        "Delete selected objects",
        "Supprimer les objets sélectionnés",
    ),
    (
        "Design a custom label or prepare a downloaded shipping label.",
        "Concevez votre propre étiquette ou préparez une étiquette d'expédition téléchargée.",
    ),
    (
        "Design and prepare thermal labels for Linux.",
        "Concevez et préparez des étiquettes thermiques pour Linux.",
    ),
    ("Discard", "Abandonner"),
    (
        "Drag to draw. Drag inside to move; drag a corner to resize. Crop coordinates refer to the original page.",
        "Faites glisser pour dessiner. Glissez à l'intérieur pour déplacer, un coin pour redimensionner. Les coordonnées du recadrage se rapportent à la page d'origine.",
    ),
    (
        "Drag to move · square to resize · circle to rotate · Shift: 15°",
        "Glisser pour déplacer · carré pour redimensionner · cercle pour pivoter · Maj : 15°",
    ),
    (
        "Drag to rotate freely. Hold Shift to snap to 15°.",
        "Faites glisser pour pivoter librement. Maintenez Maj pour aimanter à 15°.",
    ),
    (
        "Driver default resolution",
        "Résolution par défaut du pilote",
    ),
    (
        "Drop PDFs or images to build a print queue",
        "Déposez des PDF ou des images pour créer une file d'impression",
    ),
    (
        "Drop a downloaded label anywhere.",
        "Déposez une étiquette téléchargée n'importe où.",
    ),
    ("Duplicate", "Dupliquer"),
    (
        "Duplicate selected objects (Ctrl+D)",
        "Dupliquer les objets sélectionnés (Ctrl+D)",
    ),
    (
        "Embedded image. Resize with W/H or the canvas handle.",
        "Image intégrée. Redimensionnez avec L/H ou la poignée du canevas.",
    ),
    (
        "Enter your real product code or reference",
        "Saisissez votre vrai code produit ou votre référence",
    ),
    (
        "Every row is checked first; nothing prints if a row has a problem.",
        "Chaque ligne est d'abord vérifiée ; rien ne s'imprime si une ligne pose problème.",
    ),
    ("Explore ThermalPrint", "Découvrir ThermalPrint"),
    (
        "Export a CSV from your spreadsheet. Column names are not case-sensitive; ${serial} counts up from the design's serial number.",
        "Exportez un CSV depuis votre tableur. Les noms de colonnes ne sont pas sensibles à la casse ; ${serial} s'incrémente à partir du numéro de série du modèle.",
    ),
    ("Export", "Exporter"),
    (
        "Export the prepared label as PDF",
        "Exporter l'étiquette préparée en PDF",
    ),
    (
        "Export the prepared label as PNG",
        "Exporter l'étiquette préparée en PNG",
    ),
    ("File", "Fichier"),
    ("Filled", "Rempli"),
    (
        "Fit label to canvas and reset panning",
        "Ajuster l'étiquette au canevas et réinitialiser le déplacement",
    ),
    (
        "Fit label to the preview area",
        "Ajuster l'étiquette à la zone d'aperçu",
    ),
    ("Fit within paper", "Adapter au papier"),
    ("Focus canvas", "Focus sur le canevas"),
    ("Font size in points", "Taille de police en points"),
    ("Forward", "Vers l'avant"),
    (
        "From any marketplace or carrier. No account needed.",
        "De n'importe quelle place de marché ou transporteur. Aucun compte requis.",
    ),
    (
        "Hide navigation and properties to expand the canvas",
        "Masquer la navigation et les propriétés pour agrandir le canevas",
    ),
    ("Horizontal", "Horizontal"),
    (
        "Horizontal print offset in millimeters",
        "Décalage horizontal d'impression en millimètres",
    ),
    (
        "I checked the crop and barcode edges",
        "J'ai vérifié le recadrage et les bords du code-barres",
    ),
    (
        "If the loaded paper matches the printer media, match the output below. Otherwise select media that matches the label size you want.",
        "Si le papier chargé correspond au support de l'imprimante, adaptez la sortie ci-dessous. Sinon, choisissez un support correspondant à la taille d'étiquette souhaitée.",
    ),
    (
        "If the loaded paper matches the printer media, use the button below. To keep the current label size, choose matching media in Print Center instead.",
        "Si le papier chargé correspond au support de l'imprimante, utilisez le bouton ci-dessous. Pour conserver la taille actuelle de l'étiquette, choisissez plutôt un support correspondant dans le Centre d'impression.",
    ),
    ("Job status", "État du travail"),
    ("Label", "Étiquette"),
    (
        "Label height in millimeters",
        "Hauteur de l'étiquette en millimètres",
    ),
    ("Label name", "Nom de l'étiquette"),
    (
        "Label setup (no selection)",
        "Réglages de l'étiquette (aucune sélection)",
    ),
    ("Label stock", "Support d'étiquettes"),
    (
        "Label width in millimeters",
        "Largeur de l'étiquette en millimètres",
    ),
    ("Labels, ready to go.", "Étiquettes prêtes à l'emploi."),
    ("Left", "Gauche"),
    ("MIT License", "Licence MIT"),
    ("Main menu", "Menu principal"),
    (
        "Match label to printer media",
        "Adapter l'étiquette au support de l'imprimante",
    ),
    (
        "Match output to printer media",
        "Adapter la sortie au support de l'imprimante",
    ),
    ("More from the author", "Du même auteur"),
    (
        "Move label earlier in the print queue",
        "Déplacer l'étiquette plus tôt dans la file d'impression",
    ),
    (
        "Move label later in the print queue",
        "Déplacer l'étiquette plus tard dans la file d'impression",
    ),
    (
        "Move object one layer backward",
        "Reculer l'objet d'un plan",
    ),
    ("Move object one layer forward", "Avancer l'objet d'un plan"),
    ("New label", "Nouvelle étiquette"),
    ("No folder chosen", "Aucun dossier choisi"),
    (
        "No printer selected. Export is available without a printer.",
        "Aucune imprimante sélectionnée. L'export reste possible sans imprimante.",
    ),
    ("Object", "Objet"),
    (
        "Object height in millimeters",
        "Hauteur de l'objet en millimètres",
    ),
    (
        "Object horizontal position in millimeters",
        "Position horizontale de l'objet en millimètres",
    ),
    ("Object text", "Texte de l'objet"),
    (
        "Object vertical position in millimeters",
        "Position verticale de l'objet en millimètres",
    ),
    (
        "Object width in millimeters",
        "Largeur de l'objet en millimètres",
    ),
    ("Open", "Ouvrir"),
    (
        "Open a saved label design",
        "Ouvrir un modèle d'étiquette enregistré",
    ),
    ("Open import queue", "Ouvrir la file d'importation"),
    (
        "Open imported labels for review",
        "Ouvrir les étiquettes importées pour les vérifier",
    ),
    (
        "Open this label for review",
        "Ouvrir cette étiquette pour la vérifier",
    ),
    ("Original / crop", "Original / recadrage"),
    (
        "Output label proportions",
        "Proportions de l'étiquette de sortie",
    ),
    ("Output label size", "Taille de l'étiquette de sortie"),
    ("Output settings", "Réglages de sortie"),
    ("Page", "Page"),
    ("Prepare your label", "Préparez votre étiquette"),
    ("Preview row", "Aperçu de la ligne"),
    ("Preview row number", "Numéro de ligne de l'aperçu"),
    (
        "Preview the final label as it will print",
        "Aperçu de l'étiquette finale telle qu'elle sera imprimée",
    ),
    ("Print", "Imprimer"),
    ("Print Center", "Centre d'impression"),
    ("Print alignment test", "Imprimer un test d'alignement"),
    ("Print label   Ctrl+P", "Imprimer l'étiquette   Ctrl+P"),
    (
        "Print label using the selected printer",
        "Imprime l'étiquette avec l'imprimante sélectionnée",
    ),
    (
        "Print one label per row of a spreadsheet, filling ${column} placeholders",
        "Imprime une étiquette par ligne d'un tableur en remplissant les champs ${column}",
    ),
    ("Print using raw ZPL", "Imprimer en ZPL brut"),
    ("Printer & media", "Imprimante et support"),
    ("Printer queue", "File de l'imprimante"),
    ("Printer resolution", "Résolution de l'imprimante"),
    ("Printer status", "État de l'imprimante"),
    ("Printing terms", "Termes d'impression"),
    (
        "Prints a ruler label with a 1 mm border so you can measure the offset",
        "Imprime une étiquette-règle avec une bordure de 1 mm pour mesurer le décalage",
    ),
    ("Properties", "Propriétés"),
    ("Properties panel", "Panneau des propriétés"),
    (
        "Put ${column} placeholders in text, barcode, and QR content. Each row of your spreadsheet becomes one label.",
        "Placez des champs ${column} dans le texte, le code-barres et le contenu QR. Chaque ligne de votre tableur devient une étiquette.",
    ),
    ("QR code content", "Contenu du code QR"),
    ("QR content", "Contenu QR"),
    ("Queue & diagnostics", "File et diagnostic"),
    ("Quit", "Quitter"),
    (
        "Raw ZPL: verify printer compatibility and loaded label size.",
        "ZPL brut : vérifiez la compatibilité de l'imprimante et la taille d'étiquette chargée.",
    ),
    ("Recent documents", "Documents récents"),
    ("Recent prints", "Impressions récentes"),
    ("Redo (Ctrl+Y)", "Rétablir (Ctrl+Y)"),
    ("Refresh printers", "Actualiser les imprimantes"),
    (
        "Remove this label from the import queue",
        "Retirer cette étiquette de la file d'importation",
    ),
    (
        "Requires a ZPL-compatible printer and raw CUPS queue.",
        "Nécessite une imprimante compatible ZPL et une file CUPS brute.",
    ),
    ("Reset offset", "Réinitialiser le décalage"),
    (
        "Reset the crop to use the entire original page",
        "Réinitialiser le recadrage pour utiliser toute la page d'origine",
    ),
    ("Reset to entire page", "Rétablir la page entière"),
    ("Right", "Droite"),
    ("Rotate", "Pivoter"),
    ("Rotation", "Rotation"),
    ("Save as…", "Enregistrer sous…"),
    ("Save changes?", "Enregistrer les modifications ?"),
    ("Save design (Ctrl+S)", "Enregistrer le modèle (Ctrl+S)"),
    (
        "Save your changes to keep them, or discard them to continue.",
        "Enregistrez vos modifications pour les conserver, ou abandonnez-les pour continuer.",
    ),
    (
        "Select an object on the canvas.",
        "Sélectionnez un objet sur le canevas.",
    ),
    (
        "Select label object",
        "Sélectionner un objet de l'étiquette",
    ),
    (
        "Select printer media in Print Center before printing.",
        "Choisissez le support de l'imprimante dans le Centre d'impression avant d'imprimer.",
    ),
    (
        "Set the canvas dimensions to the selected printer stock. Object sizes and positions are preserved; Ctrl+Z undoes this change.",
        "Règle les dimensions du canevas sur le support d'imprimante choisi. Les tailles et positions des objets sont conservées ; Ctrl+Z annule ce changement.",
    ),
    (
        "Set up your printer and check your next print run.",
        "Configurez votre imprimante et vérifiez votre prochaine impression.",
    ),
    (
        "Settings above are remembered for each printer. Use the offset if labels print slightly off-center.",
        "Les réglages ci-dessus sont mémorisés pour chaque imprimante. Utilisez le décalage si les étiquettes s'impriment légèrement décentrées.",
    ),
    (
        "Shape stroke width in millimeters",
        "Épaisseur du trait de la forme en millimètres",
    ),
    (
        "Shift-click to select multiple objects. Use Ctrl+C/V, Ctrl+D, Delete, Ctrl+Z/Y.",
        "Maj+clic pour sélectionner plusieurs objets. Utilisez Ctrl+C/V, Ctrl+D, Suppr, Ctrl+Z/Y.",
    ),
    ("Show grid", "Afficher la grille"),
    (
        "Show value below bars",
        "Afficher la valeur sous les barres",
    ),
    ("Snap to grid", "Aimanter à la grille"),
    (
        "Text · ${date} and ${serial} are dynamic",
        "Texte · ${date} et ${serial} sont dynamiques",
    ),
    ("Thermal label", "Étiquette thermique"),
    (
        "These dimensions control the output and stock preview. Match them to the printer media size above.",
        "Ces dimensions contrôlent la sortie et l'aperçu du support. Faites-les correspondre à la taille de support de l'imprimante ci-dessus.",
    ),
    (
        "This row cannot be previewed. Printing will report the problem.",
        "Cette ligne ne peut pas être prévisualisée. L'impression signalera le problème.",
    ),
    ("Undo (Ctrl+Z)", "Annuler (Ctrl+Z)"),
    (
        "Update the output width and height to match the selected printer stock",
        "Met à jour la largeur et la hauteur de sortie selon le support d'imprimante choisi",
    ),
    ("Vertical", "Vertical"),
    (
        "Vertical print offset in millimeters",
        "Décalage vertical d'impression en millimètres",
    ),
    ("View", "Affichage"),
    (
        "View the next label (Alt+Right)",
        "Afficher l'étiquette suivante (Alt+Right)",
    ),
    (
        "View the original document and adjust the crop",
        "Voir le document d'origine et ajuster le recadrage",
    ),
    (
        "View the previous label (Alt+Left)",
        "Afficher l'étiquette précédente (Alt+Left)",
    ),
    ("WORKSPACE", "ESPACE DE TRAVAIL"),
    ("Watch a folder", "Surveiller un dossier"),
    (
        "Watch for new labels",
        "Surveiller les nouvelles étiquettes",
    ),
    (
        "Your label has unsaved changes.",
        "Votre étiquette a des modifications non enregistrées.",
    ),
    ("Your label workspace", "Votre espace d'étiquettes"),
    ("Home", "Accueil"),
    ("Label Designer", "Concepteur d'étiquettes"),
    ("Import & Convert", "Importer et convertir"),
    (
        "Open your workspace and label templates",
        "Ouvrir votre espace de travail et les modèles d'étiquettes",
    ),
    (
        "Create and edit a custom label",
        "Créer et modifier une étiquette personnalisée",
    ),
    (
        "Import, crop, and prepare existing labels",
        "Importer, recadrer et préparer des étiquettes existantes",
    ),
    (
        "Configure printer, stock, and queue settings",
        "Configurer l'imprimante, le support et la file",
    ),
    ("Create a label", "Créer une étiquette"),
    (
        "Build something of your own",
        "Créez quelque chose de personnel",
    ),
    (
        "Add text, shapes, barcodes and QR codes to a precisely sized canvas.",
        "Ajoutez du texte, des formes, des codes-barres et des codes QR à un canevas aux dimensions précises.",
    ),
    ("Create new label", "Créer une nouvelle étiquette"),
    ("Import & convert", "Importer et convertir"),
    (
        "Get existing labels ready to print",
        "Préparez des étiquettes existantes pour l'impression",
    ),
    (
        "Open PDFs or images, remove instructions, review crops, then print in order.",
        "Ouvrez des PDF ou des images, retirez les instructions, vérifiez les recadrages, puis imprimez dans l'ordre.",
    ),
    ("Open existing labels", "Ouvrir des étiquettes existantes"),
    ("Project & source code", "Projet et code source"),
    (
        "Documentation, releases, and source",
        "Documentation, versions et code",
    ),
    ("Report an issue", "Signaler un problème"),
    (
        "Share a bug or suggest an improvement",
        "Signalez un bogue ou suggérez une amélioration",
    ),
    ("Created by", "Créé par"),
    (
        "Free to use, modify, and share",
        "Libre d'utilisation, de modification et de partage",
    ),
    ("Thermal labels", "Étiquettes thermiques"),
    ("Linux app", "Application Linux"),
    ("Open source", "Open source"),
    ("Version copied", "Version copiée"),
    (
        "Power tools for your desktop",
        "Des outils puissants pour votre bureau",
    ),
    (
        "Battery health for GNOME",
        "Santé de la batterie pour GNOME",
    ),
    ("Version", "Version"),
    ("Batch print from CSV", "Impression par lot depuis un CSV"),
    ("Batch print from CSV…", "Impression par lot depuis un CSV…"),
    ("Preparing", "Préparation"),
    (
        "Wait for the current operation to finish.",
        "Attendez la fin de l'opération en cours.",
    ),
    ("Add a label", "Ajouter une étiquette"),
    (
        "Import a PDF or image to start preparing a print.",
        "Importez un PDF ou une image pour commencer à préparer une impression.",
    ),
    ("Adjust the layout", "Ajuster la mise en page"),
    (
        "The label would be clipped. Use Fit within paper or change the output dimensions.",
        "L'étiquette serait coupée. Utilisez « Adapter au papier » ou modifiez les dimensions de sortie.",
    ),
    ("Review the crop", "Vérifier le recadrage"),
    (
        "Check the original crop and barcode edges, then confirm the review in Extract.",
        "Vérifiez le recadrage d'origine et les bords du code-barres, puis confirmez la vérification dans Extraire.",
    ),
    ("Preparing preview", "Préparation de l'aperçu"),
    (
        "The label preview is being generated.",
        "L'aperçu de l'étiquette est en cours de génération.",
    ),
    ("Choose a printer", "Choisissez une imprimante"),
    (
        "Select an available printer before printing. You can still export a prepared label.",
        "Sélectionnez une imprimante disponible avant d'imprimer. Vous pouvez toujours exporter une étiquette préparée.",
    ),
    ("Choose printer media", "Choisissez le support d'impression"),
    (
        "Select a supported media size that matches the loaded labels.",
        "Sélectionnez une taille de support compatible correspondant aux étiquettes chargées.",
    ),
    (
        "Label size differs from printer media",
        "La taille de l'étiquette diffère du support de l'imprimante",
    ),
    ("Ready to submit", "Prêt à envoyer"),
    ("Printer configured", "Imprimante configurée"),
    ("Verify the loaded stock", "Vérifiez le support chargé"),
    (
        "The driver does not report dimensions for this media. Check that the loaded labels match the output size.",
        "Le pilote ne signale pas les dimensions de ce support. Vérifiez que les étiquettes chargées correspondent à la taille de sortie.",
    ),
    (
        "Crop reviewed and output prepared. Check the loaded stock before printing.",
        "Recadrage vérifié et sortie préparée. Vérifiez le support chargé avant d'imprimer.",
    ),
    (
        "Media matches the output dimensions. Import or design a label to print.",
        "Le support correspond aux dimensions de sortie. Importez ou concevez une étiquette à imprimer.",
    ),
    ("Blank custom", "Vierge"),
    ("Shipping 4 × 6", "Expédition 4 × 6"),
    ("Return address", "Adresse de retour"),
    ("Product barcode", "Code-barres produit"),
    ("Inventory", "Inventaire"),
    ("QR code", "Code QR"),
    ("Asset tracking", "Suivi des équipements"),
    ("Shelf", "Étagère"),
    ("Price tag", "Étiquette de prix"),
    ("Name label", "Étiquette de nom"),
    ("Fragile", "Fragile"),
    ("Help", "Aide"),
    ("Help & Manual", "Aide et manuel"),
    ("Tray icon restored", "Icône de notification rétablie"),
    (
        "Open the user manual in your browser (F1)",
        "Ouvre le manuel d'utilisation dans votre navigateur (F1)",
    ),
    ("Save as template…", "Enregistrer comme modèle…"),
    (
        "Keep this design in your Templates menu, named after the label",
        "Garde ce modèle dans ton menu Modèles, avec le nom de l'étiquette",
    ),
    ("Smart guides", "Repères intelligents"),
    (
        "Snap to other objects' edges and centers while dragging",
        "S'aimante aux bords et centres des autres objets pendant le déplacement",
    ),
    ("Show printable area", "Afficher la zone imprimable"),
    (
        "Shade the border your printer cannot print on",
        "Grise la bordure que ton imprimante ne peut pas imprimer",
    ),
    (
        "These objects reach into the printer's unprintable border and may be cut off:",
        "Ces objets touchent la bordure non imprimable de l'imprimante et peuvent être coupés :",
    ),
    ("Your templates", "Tes modèles"),
    ("Remove this template", "Retirer ce modèle"),
    ("Grid size", "Taille de la grille"),
    (
        "Grid size in millimeters",
        "Taille de la grille en millimètres",
    ),
    ("Align", "Aligner"),
    ("Distribute evenly", "Répartir uniformément"),
    ("Center on label", "Centrer sur l'étiquette"),
    ("Lock all", "Tout verrouiller"),
    ("Unlock all", "Tout déverrouiller"),
    ("Rotate 90°", "Pivoter de 90°"),
    (
        "Turn the object a quarter turn clockwise",
        "Fait pivoter l'objet d'un quart de tour vers la droite",
    ),
    ("Flip horizontally", "Retourner horizontalement"),
    ("Flip vertically", "Retourner verticalement"),
    ("Lock position and size", "Verrouiller position et taille"),
    ("Italic", "Italique"),
    ("Inverse", "Inversé"),
    (
        "White letters on a black box",
        "Lettres blanches sur un cadre noir",
    ),
    ("Wrap long lines", "Retour à la ligne automatique"),
    ("Shrink to fit the box", "Réduire pour tenir dans le cadre"),
    ("Line spacing", "Interligne"),
    ("Line spacing multiplier", "Multiplicateur d'interligne"),
    ("Text size", "Taille du texte"),
    (
        "Readable text size in points",
        "Taille du texte lisible en points",
    ),
    ("Text above the bars", "Texte au-dessus des barres"),
    (
        "QR error correction level",
        "Niveau de correction d'erreurs du QR",
    ),
    ("Contrast", "Contraste"),
    (
        "Image contrast in percent",
        "Contraste de l'image en pourcentage",
    ),
    ("Cut-off", "Seuil"),
    (
        "Pixels darker than this print black",
        "Les pixels plus sombres que ceci s'impriment en noir",
    ),
    ("Invert black and white", "Inverser noir et blanc"),
    (
        "Reset image adjustments",
        "Réinitialiser les réglages de l'image",
    ),
    (
        "Spacing of the guide grid. Snapping moves in steps of a fifth of it.",
        "Espacement de la grille. L'aimantation avance par pas d'un cinquième.",
    ),
    (
        "A locked object cannot be moved, resized, rotated, or deleted from the canvas",
        "Un objet verrouillé ne peut pas être déplacé, redimensionné, pivoté ni supprimé depuis le canevas",
    ),
    (
        "Break lines at spaces so they fit the box width",
        "Coupe les lignes aux espaces pour qu'elles tiennent dans la largeur du cadre",
    ),
    (
        "Make the text smaller until all of it fits inside the box",
        "Réduit le texte jusqu'à ce qu'il tienne entièrement dans le cadre",
    ),
    (
        "More error correction survives more damage but makes a denser code",
        "Plus de correction d'erreurs résiste à plus de dégâts mais donne un code plus dense",
    ),
    (
        "How the image is reduced to black and white",
        "Comment l'image est réduite en noir et blanc",
    ),
    ("Align left", "Aligner à gauche"),
    ("Align center", "Aligner au centre"),
    ("Align right", "Aligner à droite"),
    ("Align top", "Aligner en haut"),
    ("Align middle", "Aligner au milieu"),
    ("Align bottom", "Aligner en bas"),
    ("Distribute horizontally", "Répartir horizontalement"),
    ("Distribute vertically", "Répartir verticalement"),
    (
        "Center horizontally on label",
        "Centrer horizontalement sur l'étiquette",
    ),
    (
        "Center vertically on label",
        "Centrer verticalement sur l'étiquette",
    ),
    (
        "Lock all selected objects",
        "Verrouille tous les objets sélectionnés",
    ),
    (
        "Unlock all selected objects",
        "Déverrouille tous les objets sélectionnés",
    ),
    (
        "Return the image mode, contrast, and cut-off to their defaults",
        "Remet le mode d'image, le contraste et le seuil à leurs valeurs par défaut",
    ),
    ("Website", "Site web"),
    ("Wi-Fi", "Wi-Fi"),
    ("Contact card", "Carte de contact"),
    ("Email", "E-mail"),
    ("Phone number", "Numéro de téléphone"),
    ("Build the content", "Créer le contenu"),
    ("Kind of QR content", "Type de contenu QR"),
    ("Web address", "Adresse web"),
    ("Network name", "Nom du réseau"),
    ("Password", "Mot de passe"),
    ("WPA / WPA2", "WPA / WPA2"),
    ("WEP", "WEP"),
    ("No password", "Sans mot de passe"),
    ("Hidden network", "Réseau masqué"),
    ("Name", "Nom"),
    ("Organization", "Organisation"),
    ("Phone", "Téléphone"),
    ("Email address", "Adresse e-mail"),
    ("Address", "Adresse"),
    ("Subject", "Objet"),
    ("Message", "Message"),
    ("Use this content", "Utiliser ce contenu"),
    (
        "Replace the QR content with what you entered",
        "Remplace le contenu du QR par ce que tu as saisi",
    ),
    (
        "Unsaved changes from your last session were found:",
        "Des modifications non enregistrées de ta dernière session ont été trouvées :",
    ),
    ("Restore", "Restaurer"),
    (
        "Reopen the design from your last session as an unsaved copy",
        "Rouvre le modèle de ta dernière session comme copie non enregistrée",
    ),
    ("Delete the recovered copy", "Supprime la copie récupérée"),
    ("Template gallery", "Galerie de modèles"),
    ("Search", "Rechercher"),
    ("Search templates", "Rechercher des modèles"),
    (
        "Nothing saved yet. Use File → Save as template… to add one.",
        "Rien d'enregistré pour l'instant. Utilise Fichier → Enregistrer comme modèle… pour en ajouter un.",
    ),
    ("Print on a sheet", "Imprimer sur une feuille"),
    ("Paper", "Papier"),
    ("Resolution", "Résolution"),
    ("Columns", "Colonnes"),
    ("Rows", "Lignes"),
    ("Page margin", "Marge de page"),
    ("Page margin in millimeters", "Marge de page en millimètres"),
    ("Gap between labels", "Espace entre les étiquettes"),
    (
        "Gap between labels in millimeters",
        "Espace entre les étiquettes en millimètres",
    ),
    ("How many labels", "Combien d'étiquettes"),
    ("Number of labels", "Nombre d'étiquettes"),
    ("Fit as many as possible", "Autant que possible"),
    (
        "Choose the most columns and rows of this label size that fit the page",
        "Choisit le plus de colonnes et de lignes de cette taille d'étiquette qui tiennent sur la page",
    ),
    (
        "The selected printer does not list this paper. You can still save a PDF.",
        "L'imprimante sélectionnée ne propose pas ce papier. Tu peux quand même enregistrer un PDF.",
    ),
    ("Save as PDF", "Enregistrer en PDF"),
    (
        "Save the sheet as a PDF file",
        "Enregistre la feuille en fichier PDF",
    ),
    ("Print sheet", "Imprimer la feuille"),
    (
        "Send the sheet to the selected printer",
        "Envoie la feuille à l'imprimante sélectionnée",
    ),
    ("labels per page", "étiquettes par page"),
    ("page", "page"),
    ("pages", "pages"),
    ("Print a number range", "Imprimer une plage de numéros"),
    (
        "Put ${serial} in text, barcode, or QR content. Use ${serial:4} for numbers padded with zeros, like 0007. Add a prefix by typing it before the number, for example INV-${serial:4}.",
        "Mets ${serial} dans le texte, le code-barres ou le contenu QR. Utilise ${serial:4} pour des numéros complétés par des zéros, comme 0007. Ajoute un préfixe en l'écrivant avant le numéro, par exemple INV-${serial:4}.",
    ),
    ("First number", "Premier numéro"),
    ("Step", "Pas"),
    ("Step between numbers", "Pas entre les numéros"),
    ("Serials", "Numéros"),
    (
        "This design has no ${serial} yet, so every label would be the same.",
        "Ce modèle ne contient pas encore ${serial}, donc toutes les étiquettes seraient identiques.",
    ),
    ("Print labels:", "Imprimer les étiquettes :"),
    (
        "Every label is checked first; nothing prints if one has a problem.",
        "Chaque étiquette est vérifiée d'abord ; rien n'est imprimé si l'une a un problème.",
    ),
    ("Template gallery…", "Galerie de modèles…"),
    (
        "Browse the built-in templates and your own with previews",
        "Parcours les modèles intégrés et les tiens avec aperçus",
    ),
    ("Print on a sheet…", "Imprimer sur une feuille…"),
    ("Print a number range…", "Imprimer une plage de numéros…"),
    (
        "Print labels numbered by ${serial} with a start, step, and count",
        "Imprime des étiquettes numérotées avec ${serial} selon un début, un pas et un nombre",
    ),
    ("Address book", "Carnet d'adresses"),
    (
        "Nothing saved yet. Select a text object and choose Save to address book.",
        "Rien d'enregistré pour l'instant. Sélectionne un texte et choisis Enregistrer dans le carnet d'adresses.",
    ),
    ("Remove this address", "Retirer cette adresse"),
    ("Symbols", "Symboles"),
    ("Your logos", "Tes logos"),
    ("Remove this logo", "Retirer ce logo"),
    ("Symbols and logos", "Symboles et logos"),
    (
        "Auto-length (continuous roll)",
        "Longueur automatique (rouleau continu)",
    ),
    (
        "Print only as long as the content needs, plus a bottom margin. Use it with continuous label rolls.",
        "N'imprime que la longueur nécessaire au contenu, plus une marge basse. À utiliser avec des rouleaux continus.",
    ),
    ("Bottom margin", "Marge basse"),
    ("Bottom margin in millimeters", "Marge basse en millimètres"),
    ("Printed length", "Longueur imprimée"),
    (
        "Save to address book",
        "Enregistrer dans le carnet d'adresses",
    ),
    (
        "Keep this text, named after its first line, in your address book",
        "Garde ce texte, nommé d'après sa première ligne, dans ton carnet d'adresses",
    ),
    ("Font", "Police"),
    (
        "This font is not installed on this computer, so the built-in font is used.",
        "Cette police n'est pas installée sur cet ordinateur, la police intégrée est donc utilisée.",
    ),
    (
        "Add a font from a .ttf or .otf file",
        "Ajoute une police depuis un fichier .ttf ou .otf",
    ),
    ("Save as logo", "Enregistrer comme logo"),
    (
        "Keep this image under Symbols and logos so you can add it to any label",
        "Garde cette image sous Symboles et logos pour l'ajouter à n'importe quelle étiquette",
    ),
    ("Prints as", "S'imprime comme"),
    ("SSCC helper", "Aide SSCC"),
    (
        "An SSCC numbers one pallet or parcel: extension digit, your GS1 company prefix, and a serial reference. The check digit is added for you.",
        "Un SSCC numérote une palette ou un colis : chiffre d'extension, ton préfixe d'entreprise GS1 et une référence de série. Le chiffre de contrôle est ajouté pour toi.",
    ),
    ("Extension digit", "Chiffre d'extension"),
    ("Company prefix", "Préfixe d'entreprise"),
    ("Serial reference", "Référence de série"),
    ("Use this SSCC", "Utiliser ce SSCC"),
    (
        "Replace the barcode content with (00) and the SSCC",
        "Remplace le contenu du code-barres par (00) et le SSCC",
    ),
    (
        "The logistics barcode for pallets and cartons. Type GS1 data such as (01)09506000134352(10)LOT7.",
        "Le code-barres logistique pour palettes et cartons. Saisis des données GS1 comme (01)09506000134352(10)LOT7.",
    ),
    (
        "A small square symbol for GS1 data such as (01)09506000134352(10)LOT7. Put a batch or serial last.",
        "Un petit symbole carré pour des données GS1 comme (01)09506000134352(10)LOT7. Mets le lot ou le numéro de série en dernier.",
    ),
    ("Rotate 90° when printing", "Pivoter de 90° à l'impression"),
    (
        "For wide content: design the label turned on its side so the text reads straight, and it is rotated 90° when it prints, to fit the label stock as loaded.",
        "Pour un contenu large : concevez l'étiquette tournée sur le côté pour lire le texte droit ; à l'impression, elle pivote de 90° pour tenir sur les étiquettes telles qu'elles sont chargées.",
    ),
    (
        "Rotated 90° counterclockwise to print on",
        "Pivotée de 90° dans le sens antihoraire pour imprimer sur",
    ),
    (
        "Rotated 90° clockwise to print on",
        "Pivotée de 90° dans le sens horaire pour imprimer sur",
    ),
    ("Rotate the other way", "Pivoter dans l'autre sens"),
    (
        "Which way the design faces on the printed label. Check “As printed” beside the canvas.",
        "Dans quel sens le design apparaît sur l'étiquette imprimée. Vérifiez « Imprimée » à côté du canevas.",
    ),
    (
        "Not available with rotated printing.",
        "Indisponible avec l'impression pivotée.",
    ),
    ("Rotated printing.", "Impression pivotée."),
    (
        "You are designing the label on its side. When it prints, it is rotated 90° counterclockwise to fit",
        "Vous concevez l'étiquette sur le côté. À l'impression, elle pivote de 90° dans le sens antihoraire pour tenir sur",
    ),
    (
        "You are designing the label on its side. When it prints, it is rotated 90° clockwise to fit",
        "Vous concevez l'étiquette sur le côté. À l'impression, elle pivote de 90° dans le sens horaire pour tenir sur",
    ),
    (
        "See “As printed” in the corner.",
        "Voir « Imprimée » dans le coin.",
    ),
    ("As printed", "Imprimée"),
    (
        "Rotated printing: Print rotates it 90° to fit",
        "Impression pivotée : Imprimer la fait pivoter de 90° pour tenir sur",
    ),
    ("Default (DejaVu Sans)", "Par défaut (DejaVu Sans)"),
    (
        "Built in · prints the same everywhere",
        "Intégrées · s'impriment pareil partout",
    ),
    ("Your fonts", "Vos polices"),
    ("On this computer", "Sur cet ordinateur"),
    ("Search fonts", "Rechercher des polices"),
    ("No fonts match.", "Aucune police ne correspond."),
    (
        "This font has no bold style. Choose a bold font instead.",
        "Cette police n'a pas de style gras. Choisissez plutôt une police grasse.",
    ),
    (
        "Another computer needs this font installed to print the label the same way.",
        "Un autre ordinateur doit avoir cette police installée pour imprimer l'étiquette à l'identique.",
    ),
    (
        "Condensed (Noto Sans Condensed)",
        "Étroite (Noto Sans Condensed)",
    ),
    (
        "Monospace (DejaVu Sans Mono)",
        "Chasse fixe (DejaVu Sans Mono)",
    ),
    ("Serif (DejaVu Serif)", "À empattements (DejaVu Serif)"),
    (
        "Headline (Noto Sans Display ExtraCondensed Black)",
        "Titre (Noto Sans Display ExtraCondensed Black)",
    ),
    ("Rounded (Varela Round)", "Arrondie (Varela Round)"),
    ("Choose a font", "Choisir une police"),
    // Label footer
    ("Label footer", "Pied d'étiquette"),
    (
        "The footer QR code was left out because there is not enough blank space at the bottom of this label for it. The message still prints.",
        "Le code QR du pied a été omis car il n'y a pas assez d'espace libre en bas de cette étiquette. Le message s'imprime quand même.",
    ),
    ("Print a footer message", "Imprimer un message de pied"),
    ("Footer message", "Message de pied"),
    ("QR code link or text", "Lien ou texte du code QR"),
    ("Footer QR code content", "Contenu du code QR du pied"),
    (
        "Enter a message to print the footer. The QR code prints only with a message.",
        "Saisissez un message pour imprimer le pied. Le code QR ne s'imprime qu'avec un message.",
    ),
    (
        "Scan a test print with your phone before printing a batch.",
        "Scannez une impression d'essai avec votre téléphone avant d'imprimer un lot.",
    ),
    ("Reset footer", "Réinitialiser le pied"),
    (
        "The QR text is too long to encode. Shorten it.",
        "Le texte du QR est trop long pour être encodé. Raccourcissez-le.",
    ),
    (
        "The footer QR code was left out because it would be large on this short label. The message still prints.",
        "Le code QR du pied a été omis car il serait grand sur cette étiquette courte. Le message s'imprime quand même.",
    ),
    // 0.15.0: designer footer, overlap warning, symbols
    (
        "Add a QR code before the message",
        "Ajouter un code QR avant le message",
    ),
    (
        "Turn the footer off and restore the default message, QR code link, and alignment",
        "Désactiver le pied et rétablir le message, le lien du code QR et l'alignement par défaut",
    ),
    (
        "The footer was left out: part of the design is in the blank space it needs at the bottom of the label. Move objects up, choose another alignment, or turn the footer off. The label size is unchanged.",
        "Le pied a été omis : une partie du design occupe l'espace vide dont il a besoin en bas de l'étiquette. Remontez des objets, choisissez un autre alignement ou désactivez le pied. La taille de l'étiquette ne change pas.",
    ),
    (
        "The footer was left out: the label is too small for it inside the printable area.",
        "Le pied a été omis : l'étiquette est trop petite pour lui dans la zone imprimable.",
    ),
    (
        "Adds a small message, and if you like a QR code, along the bottom of labels you design here when you print them. Imported shipping labels never get a footer.",
        "Ajoute un petit message et, si vous le souhaitez, un code QR en bas des étiquettes conçues ici lorsque vous les imprimez. Les étiquettes d'expédition importées n'ont jamais de pied.",
    ),
    (
        "Choose where the whole footer sits along the bottom edge. Pick a side where your design leaves blank space.",
        "Choisissez où se place tout le pied le long du bord inférieur. Prenez un côté où votre design laisse de l'espace vide.",
    ),
    (
        "The label size and printable area never change. If that spot is not blank or is outside the printable area, the QR code, or the whole footer, is left out and a note says why. Exports do not include the footer.",
        "La taille de l'étiquette et la zone imprimable ne changent jamais. Si cet endroit n'est pas vide ou sort de la zone imprimable, le code QR, ou tout le pied, est omis et une note en explique la raison. Les exports n'incluent pas le pied.",
    ),
    (
        "Put the whole footer, QR code included, at the bottom-left",
        "Placer tout le pied, code QR compris, en bas à gauche",
    ),
    (
        "Put the whole footer, QR code included, at the bottom center",
        "Placer tout le pied, code QR compris, en bas au centre",
    ),
    (
        "Put the whole footer, QR code included, at the bottom-right",
        "Placer tout le pied, code QR compris, en bas à droite",
    ),
    (
        "The footer fits: the canvas shows it where it will print.",
        "Le pied tient : le canevas le montre là où il sera imprimé.",
    ),
    ("Footer", "Pied"),
    (
        "Show the label footer settings in the properties panel",
        "Afficher les réglages du pied d'étiquette dans le panneau des propriétés",
    ),
    (
        "Print is off while a barcode or QR code overlaps another object. Move or resize them so they no longer touch, and Print turns back on.",
        "L'impression est désactivée tant qu'un code-barres ou un code QR chevauche un autre objet. Déplacez-les ou redimensionnez-les pour qu'ils ne se touchent plus, et l'impression se réactive.",
    ),
    (
        "Overlap: these codes touch another object, so Print is off until they are apart. Move or resize them:",
        "Chevauchement : ces codes touchent un autre objet, l'impression est donc désactivée jusqu'à ce qu'ils soient séparés. Déplacez-les ou redimensionnez-les :",
    ),
    ("This side up", "Haut"),
    ("Keep dry", "Craint l'humidité"),
    ("Recycling", "Recyclage"),
    ("Hazard", "Danger"),
    ("Do not stack", "Ne pas empiler"),
    ("Keep cool", "Conserver au frais"),
    ("Keep out of sunlight", "Protéger du soleil"),
    ("Package", "Colis"),
    // Interface reorganization
    (
        "Check every barcode and QR code for content, size, clear space, overlap, and clipping",
        "Vérifier le contenu, la taille, la zone de silence, les chevauchements et les coupures de chaque code-barres et code QR",
    ),
    (
        "Choose a printer on the Printer tab first. Stock profiles belong to a printer.",
        "Choisissez d'abord une imprimante dans l'onglet Imprimante. Les profils d'étiquettes appartiennent à une imprimante.",
    ),
    ("Compact layout", "Affichage compact"),
    ("Hide other notes", "Masquer les autres notes"),
    ("Label footer…", "Pied d'étiquette…"),
    ("Label footer (on)…", "Pied d'étiquette (activé)…"),
    (
        "Labels you print are listed here, so you can open or reprint them.",
        "Les étiquettes imprimées apparaissent ici pour que vous puissiez les ouvrir ou les réimprimer.",
    ),
    ("More notes", "Plus de notes"),
    (
        "Tighter spacing, so more fits on a small screen",
        "Espacement réduit pour en afficher davantage sur un petit écran",
    ),
    ("Zoom to 100%", "Zoom à 100 %"),
    ("Printer", "Imprimante"),
    ("Automation", "Automatisation"),
    (
        "Place copies of this label on one page of any paper size your printer supports, for an office printer or a sheet of sticky labels. ${serial} counts up from one label to the next.",
        "Place des copies de cette étiquette sur une page de n’importe quel format pris en charge par votre imprimante, pour une imprimante de bureau ou une planche d’étiquettes adhésives. ${serial} augmente d’une étiquette à la suivante.",
    ),
    ("Printer paper size", "Format de papier de l’imprimante"),
    ("Custom size", "Format personnalisé"),
    (
        "Place several labels on one page of any paper size",
        "Placer plusieurs étiquettes sur une page de n’importe quel format",
    ),
    (
        "Choose the paper or labels loaded in the printer. Custom uses the output dimensions.",
        "Choisissez le papier ou les étiquettes chargés dans l’imprimante. Personnalisé utilise les dimensions de sortie.",
    ),
    (
        "Choose a printer and paper size, check alignment, and save it all as a stock profile you can load later.",
        "Choisissez une imprimante et un format de papier, vérifiez l’alignement et enregistrez le tout comme profil à recharger plus tard.",
    ),
    ("Use the paper size", "Utiliser le format du papier"),
    ("Choose printer", "Choisir l’imprimante"),
    ("Looking for printers…", "Recherche des imprimantes…"),
    (
        "No printers found. Add a printer in your system settings, then refresh.",
        "Aucune imprimante trouvée. Ajoutez-en une dans les réglages du système, puis actualisez.",
    ),
    (
        "Choose a printer first.",
        "Choisissez d’abord une imprimante.",
    ),
    ("Paper or label size", "Format du papier ou de l’étiquette"),
    ("Custom dimensions", "Dimensions personnalisées"),
    (
        "This printer does not list its paper sizes. Configure them in its driver.",
        "Cette imprimante n’indique pas ses formats de papier. Configurez-les dans son pilote.",
    ),
    ("Saves:", "Enregistre :"),
    (
        "For example: 4 × 6 shipping labels",
        "Par exemple : étiquettes d’expédition 4 × 6",
    ),
    ("Replace stock profile", "Remplacer le profil de support"),
    ("Stock profile saved", "Profil de support enregistré"),
    (
        "A profile with this name already exists for this printer. Saving replaces it.",
        "Un profil portant ce nom existe déjà pour cette imprimante. L’enregistrer le remplace.",
    ),
    (
        "You have reached the limit of saved stock profiles. Delete one to add another.",
        "Vous avez atteint la limite de profils enregistrés. Supprimez-en un pour en ajouter un autre.",
    ),
    (
        "A stock profile remembers a printer with its paper size, resolution, and alignment. Load one to switch to all of it at once.",
        "Un profil de support mémorise une imprimante avec son format de papier, sa résolution et son alignement. Chargez-en un pour tout changer d’un coup.",
    ),
    (
        "No saved stock yet. Set up the printer and paper, then save them below.",
        "Aucun support enregistré. Configurez l’imprimante et le papier, puis enregistrez-les ci-dessous.",
    ),
    ("Load", "Charger"),
    (
        "This printer is not connected",
        "Cette imprimante n’est pas connectée",
    ),
    ("Delete stock profile?", "Supprimer le profil de support ?"),
    ("Stock profile deleted", "Profil de support supprimé"),
    (
        "Choose the printer you will print on.",
        "Choisissez l’imprimante sur laquelle vous allez imprimer.",
    ),
    (
        "Choose the size loaded in the printer. Label printers list their label sizes; office printers list every paper size they support, such as Letter, Legal, or A4.",
        "Choisissez le format chargé dans l’imprimante. Les imprimantes d’étiquettes listent leurs formats d’étiquettes ; les imprimantes de bureau, tous les formats de papier pris en charge, comme Letter, Legal ou A4.",
    ),
    (
        "Optional. Print one test page and compare its 1 mm border to the edges. If it is off, move it with the offsets.",
        "Facultatif. Imprimez une page de test et comparez sa bordure de 1 mm aux bords. Si elle est décalée, corrigez-la avec les décalages.",
    ),
    (
        "Name this setup so you can switch back to it in one click from Label stock.",
        "Nommez cette configuration pour y revenir en un clic depuis Support d’étiquettes.",
    ),
    (
        "It is now the active profile. Switch profiles any time from Active profile at the top of Print Center.",
        "C’est maintenant le profil actif. Changez de profil à tout moment depuis Profil actif, en haut du Centre d’impression.",
    ),
    ("Finish", "Terminer"),
    ("Set up another", "En configurer un autre"),
    ("Active profile:", "Profil actif :"),
    ("Stock profile updated", "Profil de support mis à jour"),
    ("None", "Aucun"),
    (
        "Changed since it was loaded",
        "Modifié depuis son chargement",
    ),
    ("Update profile", "Mettre à jour le profil"),
    ("Active", "Actif"),
    (
        "Save the current printer, paper, resolution, and alignment into this profile",
        "Enregistrer l’imprimante, le papier, la résolution et l’alignement actuels dans ce profil",
    ),
    (
        "Changed since it was loaded. Update profile saves the changes.",
        "Modifié depuis son chargement. Mettre à jour le profil enregistre les modifications.",
    ),
    ("Saved and active:", "Enregistré et actif :"),
    (
        "Save the current setup as a new profile",
        "Enregistrer la configuration actuelle comme nouveau profil",
    ),
    ("Unsaved", "Non enregistré"),
    ("rotated printing", "impression pivotée"),
    ("{count} objects", "{count} objets"),
    ("1 copy", "1 copie"),
    ("{count} copies", "{count} copies"),
    (
        "Printer media: {media}",
        "Support de l’imprimante : {media}",
    ),
    ("Not selected", "Non sélectionné"),
    (
        "Custom — matches output",
        "Personnalisé — correspond à la sortie",
    ),
    (
        "{count} imported labels queued",
        "{count} étiquettes importées en file d’attente",
    ),
    (
        "Print queue · {count} labels",
        "File d’impression · {count} étiquettes",
    ),
    ("Label {n} of {total}", "Étiquette {n} sur {total}"),
    ("Queued labels", "Étiquettes en file d’attente"),
    (
        "Print all {count} labels in order",
        "Imprimer les {count} étiquettes dans l’ordre",
    ),
    ("Recently opened", "Ouverts récemment"),
    ("of {total}", "sur {total}"),
    (
        "Advanced layout & resolution",
        "Mise en page et résolution avancées",
    ),
    (
        "Demo workspace · All sample labels are fictional",
        "Espace de démonstration · Toutes les étiquettes d’exemple sont fictives",
    ),
    ("Automatic", "Automatique"),
    ("Manual", "Manuel"),
    ("Full page", "Page entière"),
    ("Preview", "Aperçu"),
    ("01  EXTRACT", "01  EXTRAIRE"),
    ("02  PAPER & LAYOUT", "02  PAPIER ET MISE EN PAGE"),
    ("03  PRINT & EXPORT", "03  IMPRIMER ET EXPORTER"),
    (
        "Automatically detect the label region",
        "Détecter automatiquement la zone de l’étiquette",
    ),
    (
        "Manually adjust the crop on the original document",
        "Ajuster manuellement le recadrage sur le document d’origine",
    ),
    (
        "Use the entire page without cropping",
        "Utiliser la page entière sans recadrage",
    ),
    (
        "Paper boundary · Final output uses the full-resolution render",
        "Limite du papier · La sortie finale utilise le rendu en pleine résolution",
    ),
    (
        "Original document · Green boundary is the crop · Crop before rotating",
        "Document d’origine · Le cadre vert est le recadrage · Recadrez avant de pivoter",
    ),
    (
        "Likely thermal label printer",
        "Probablement une imprimante thermique d’étiquettes",
    ),
    (
        "Likely label printer · check media",
        "Probablement une imprimante d’étiquettes · vérifiez le support",
    ),
    (
        "Supports label-sized media · verify printer type",
        "Prend en charge les formats d’étiquette · vérifiez le type d’imprimante",
    ),
    (
        "Printer type unknown · check its driver and paper",
        "Type d’imprimante inconnu · vérifiez son pilote et le papier",
    ),
    (
        "Margins & image enhancement",
        "Marges et amélioration de l’image",
    ),
    (
        "Print through the printer driver",
        "Imprimer via le pilote de l'imprimante",
    ),
    (
        "Recommended. The printer's driver handles the label size and printing.",
        "Recommandé. Le pilote de l'imprimante gère la taille de l'étiquette et l'impression.",
    ),
    ("Print using raw TSPL", "Imprimer en TSPL brut"),
    (
        "Raw TSPL: verify printer compatibility and loaded label size.",
        "TSPL brut : vérifiez la compatibilité de l'imprimante et la taille d'étiquette chargée.",
    ),
    (
        "For TSPL printers such as many 4 × 6 shipping printers. Requires a raw CUPS queue. The printer keeps its own gap calibration.",
        "Pour les imprimantes TSPL, comme de nombreuses imprimantes d'expédition 4 × 6. Nécessite une file CUPS brute. L'imprimante conserve son propre étalonnage d'espacement.",
    ),
    (
        "For TSPL printers such as many 4 × 6 shipping printers. Requires a printer that accepts RAW jobs. The printer keeps its own gap calibration.",
        "Pour les imprimantes TSPL, comme de nombreuses imprimantes d'expédition 4 × 6. Nécessite une imprimante qui accepte les travaux RAW. L'imprimante conserve son propre étalonnage d'espacement.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    const TABLES: [(&str, &[(&str, &str)]); 3] = [("Spanish", ES), ("German", DE), ("French", FR)];

    fn tokens(text: &str) -> Vec<&str> {
        let mut found = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find("${") {
            let Some(end) = rest[start..].find('}') else {
                break;
            };
            found.push(&rest[start..start + end + 1]);
            rest = &rest[start + end + 1..];
        }
        found
    }

    #[test]
    fn system_locales_pick_the_matching_language() {
        assert_eq!(detect("es_MX.UTF-8"), Resolved::Spanish);
        assert_eq!(detect("de_DE"), Resolved::German);
        assert_eq!(detect("FR_ca"), Resolved::French);
        assert_eq!(detect("fr:en"), Resolved::French);
        for other in ["en_US.UTF-8", "pt_BR", "zh_CN", "C", ""] {
            assert_eq!(detect(other), Resolved::English, "{other}");
        }
    }

    #[test]
    fn translations_apply_and_missing_ones_fall_back_to_english() {
        assert_eq!(
            translate(Resolved::Spanish, "Print Center"),
            "Centro de impresión"
        );
        assert_eq!(translate(Resolved::German, "Quit"), "Beenden");
        assert_eq!(translate(Resolved::French, "Close"), "Fermer");
        assert_eq!(translate(Resolved::English, "Quit"), "Quit");
        assert_eq!(
            translate(Resolved::French, "No such label"),
            "No such label"
        );
    }

    #[test]
    fn every_language_covers_the_same_strings_in_the_same_order() {
        fn keys<'a>(table: &[(&'a str, &'a str)]) -> Vec<&'a str> {
            table.iter().map(|(k, _)| *k).collect()
        }
        for (name, table) in TABLES {
            assert_eq!(keys(table), keys(ES), "{name}");
            let unique: std::collections::HashSet<_> = keys(table).into_iter().collect();
            assert_eq!(unique.len(), table.len(), "{name} has a duplicate key");
        }
    }

    #[test]
    fn translations_keep_placeholders_and_trailing_punctuation() {
        for (name, table) in TABLES {
            for (english, translated) in table {
                assert!(!translated.trim().is_empty(), "{name}: empty for {english}");
                assert_eq!(
                    tokens(english),
                    tokens(translated),
                    "{name}: placeholders in {english}"
                );
                assert_eq!(
                    english.ends_with('…'),
                    translated.ends_with('…'),
                    "{name}: ellipsis in {english}"
                );
                let trailing = |text: &str| text.len() - text.trim_end().len();
                assert_eq!(
                    trailing(english),
                    trailing(translated),
                    "{name}: spacing in {english}"
                );
            }
        }
    }

    #[test]
    fn every_translated_string_exists_in_the_interface_code() {
        let sources = [
            include_str!("ui.rs"),
            include_str!("ui/workflows.rs"),
            include_str!("background.rs"),
            include_str!("ui/about.rs"),
            include_str!("ui/notices.rs"),
            include_str!("ui/window_chrome.rs"),
            include_str!("designer_ui.rs"),
            include_str!("design.rs"),
            include_str!("fonts.rs"),
            include_str!("help.rs"),
            include_str!("footer.rs"),
            include_str!("design_render.rs"),
            include_str!("symbols.rs"),
            include_str!("printing.rs"),
            include_str!("ui/screenshots.rs"),
        ]
        .concat();
        let missing: Vec<_> = ES
            .iter()
            .map(|(english, _)| *english)
            .filter(|english| !sources.contains(&format!("\"{english}\"")))
            .collect();
        assert!(
            missing.is_empty(),
            "translated but not used in the code: {missing:?}"
        );
    }

    /// Every string the interface passes straight to `tr("…")` must be translated, so a new
    /// feature cannot ship with English-only labels in the other languages.
    #[test]
    fn every_interface_string_has_a_translation_in_every_language() {
        fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    sources(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && path.file_name().is_some_and(|n| n != "i18n.rs")
                {
                    out.push(path);
                }
            }
        }
        let mut files = Vec::new();
        sources(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let mut missing = Vec::new();
        for file in files {
            let code = std::fs::read_to_string(&file).unwrap();
            let bytes = code.as_bytes();
            let mut at = 0;
            while let Some(found) = code[at..].find("tr(") {
                let start = at + found;
                at = start + 3;
                let before = code[..start].chars().next_back();
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let mut i = at;
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if bytes.get(i) != Some(&b'"') {
                    continue;
                }
                let mut text = String::new();
                let mut chars = code[i + 1..].chars();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => text.push('\n'),
                            Some('"') => text.push('"'),
                            Some('\\') => text.push('\\'),
                            Some('\n') => {
                                // A line continuation skips the next line's indentation.
                                let rest = chars.as_str();
                                let trimmed = rest.trim_start();
                                chars = trimmed.chars();
                            }
                            _ => {}
                        },
                        c => text.push(c),
                    }
                }
                for (name, table) in TABLES {
                    if !table.iter().any(|(k, _)| *k == text) {
                        missing.push(format!("{name}: {text:?} ({})", file.display()));
                    }
                }
            }
        }
        assert!(
            missing.is_empty(),
            "add these strings to the ES, DE and FR tables in src/i18n.rs:\n{}",
            missing.join("\n")
        );
    }
}
