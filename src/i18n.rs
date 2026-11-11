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
fn system_locale() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty() && value != "C" && value != "POSIX")
        .unwrap_or_default()
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
        "Add new PDF and image labels automatically when they appear in a folder, such as Downloads. Files already there are ignored.",
        "Añade automáticamente las nuevas etiquetas PDF y de imagen que aparezcan en una carpeta, como Descargas. Los archivos que ya estén ahí se ignoran.",
    ),
    ("Alignment", "Alineación"),
    ("Appearance", "Apariencia"),
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
    (
        "Choose the driver media that matches your loaded labels. Custom uses the output dimensions.",
        "Elige el papel del controlador que coincida con tus etiquetas cargadas. «Personalizado» usa las dimensiones de salida.",
    ),
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
        "Custom — match output dimensions",
        "Personalizado: igualar las dimensiones de salida",
    ),
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
        "Driver media must match the paper dimensions above. Check your printer's unprintable margins.",
        "El papel del controlador debe coincidir con las dimensiones anteriores. Revisa los márgenes no imprimibles de tu impresora.",
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
    ("Focus", "Enfocar"),
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
    ("OPEN  /  CHECK  /  PRINT", "ABRIR  /  REVISAR  /  IMPRIMIR"),
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
    ("Printer media size", "Tamaño de papel de la impresora"),
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
    (
        "Universal shipping labels",
        "Etiquetas de envío universales",
    ),
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
    ("User manual", "Manual de usuario"),
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
    (
        "Place copies of this label on an A4 or Letter page, for an office printer or a sheet of sticky labels. ${serial} counts up from one label to the next.",
        "Coloca copias de esta etiqueta en una página A4 o Carta, para una impresora de oficina o una hoja de etiquetas adhesivas. ${serial} aumenta de una etiqueta a la siguiente.",
    ),
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
    (
        "Place several labels on one A4 or Letter page",
        "Coloca varias etiquetas en una página A4 o Carta",
    ),
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
    (
        "Your own fonts have no bold switch. Install a bold font file instead.",
        "Tus propias fuentes no tienen opción de negrita. Instala un archivo de fuente en negrita.",
    ),
    ("Built-in font", "Fuente incluida"),
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
];
const DE: &[(&str, &str)] = &[
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
        "Add new PDF and image labels automatically when they appear in a folder, such as Downloads. Files already there are ignored.",
        "Neue PDF- und Bild-Etiketten werden automatisch hinzugefügt, sobald sie in einem Ordner (z. B. Downloads) erscheinen. Bereits vorhandene Dateien werden ignoriert.",
    ),
    ("Alignment", "Ausrichtung"),
    ("Appearance", "Erscheinungsbild"),
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
    (
        "Choose the driver media that matches your loaded labels. Custom uses the output dimensions.",
        "Wählen Sie das Treibermedium, das zu Ihren eingelegten Etiketten passt. „Benutzerdefiniert“ verwendet die Ausgabemaße.",
    ),
    ("Clear", "Entfernen"),
    ("Clear history", "Verlauf löschen"),
    ("Close", "Schließen"),
    ("Copy version information", "Versionsinformationen kopieren"),
    ("Credits and license", "Danksagungen und Lizenz"),
    ("Custom scale", "Benutzerdefinierte Skalierung"),
    (
        "Custom — match output dimensions",
        "Benutzerdefiniert – Ausgabemaße übernehmen",
    ),
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
        "Driver media must match the paper dimensions above. Check your printer's unprintable margins.",
        "Das Treibermedium muss den obigen Papiermaßen entsprechen. Beachten Sie die nicht bedruckbaren Ränder Ihres Druckers.",
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
    ("Focus", "Fokus"),
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
    ("OPEN  /  CHECK  /  PRINT", "ÖFFNEN  /  PRÜFEN  /  DRUCKEN"),
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
    ("Printer media size", "Medienformat des Druckers"),
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
    ("Universal shipping labels", "Universelle Versandetiketten"),
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
    ("User manual", "Benutzerhandbuch"),
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
    (
        "Place copies of this label on an A4 or Letter page, for an office printer or a sheet of sticky labels. ${serial} counts up from one label to the next.",
        "Platziert Kopien dieser Etikette auf einer A4- oder Letter-Seite, für einen Bürodrucker oder einen Bogen Haftetiketten. ${serial} zählt von Etikette zu Etikette hoch.",
    ),
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
    (
        "Place several labels on one A4 or Letter page",
        "Mehrere Etiketten auf eine A4- oder Letter-Seite setzen",
    ),
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
    (
        "Your own fonts have no bold switch. Install a bold font file instead.",
        "Eigene Schriften haben keinen Fett-Schalter. Installiere stattdessen eine fette Schriftdatei.",
    ),
    ("Built-in font", "Integrierte Schrift"),
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
];
const FR: &[(&str, &str)] = &[
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
        "Add new PDF and image labels automatically when they appear in a folder, such as Downloads. Files already there are ignored.",
        "Ajoute automatiquement les nouvelles étiquettes PDF et image qui apparaissent dans un dossier, comme Téléchargements. Les fichiers déjà présents sont ignorés.",
    ),
    ("Alignment", "Alignement"),
    ("Appearance", "Apparence"),
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
    (
        "Choose the driver media that matches your loaded labels. Custom uses the output dimensions.",
        "Choisissez le support du pilote correspondant à vos étiquettes chargées. « Personnalisé » utilise les dimensions de sortie.",
    ),
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
        "Custom — match output dimensions",
        "Personnalisé — reprendre les dimensions de sortie",
    ),
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
        "Driver media must match the paper dimensions above. Check your printer's unprintable margins.",
        "Le support du pilote doit correspondre aux dimensions ci-dessus. Vérifiez les marges non imprimables de votre imprimante.",
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
    ("Focus", "Focus"),
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
    (
        "OPEN  /  CHECK  /  PRINT",
        "OUVRIR  /  VÉRIFIER  /  IMPRIMER",
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
    ("Printer media size", "Taille du support de l'imprimante"),
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
    (
        "Universal shipping labels",
        "Étiquettes d'expédition universelles",
    ),
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
    ("User manual", "Manuel d'utilisation"),
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
    (
        "Place copies of this label on an A4 or Letter page, for an office printer or a sheet of sticky labels. ${serial} counts up from one label to the next.",
        "Place des copies de cette étiquette sur une page A4 ou Letter, pour une imprimante de bureau ou une planche d'étiquettes adhésives. ${serial} augmente d'une étiquette à la suivante.",
    ),
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
    (
        "Place several labels on one A4 or Letter page",
        "Place plusieurs étiquettes sur une page A4 ou Letter",
    ),
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
    (
        "Your own fonts have no bold switch. Install a bold font file instead.",
        "Tes propres polices n'ont pas de commutateur gras. Installe plutôt un fichier de police en gras.",
    ),
    ("Built-in font", "Police intégrée"),
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
            include_str!("ui/about.rs"),
            include_str!("ui/notices.rs"),
            include_str!("ui/window_chrome.rs"),
            include_str!("designer_ui.rs"),
            include_str!("design.rs"),
            include_str!("help.rs"),
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
