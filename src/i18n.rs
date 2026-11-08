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
    ("View the next label", "Ver la etiqueta siguiente"),
    (
        "View the original document and adjust the crop",
        "Ver el documento original y ajustar el recorte",
    ),
    ("View the previous label", "Ver la etiqueta anterior"),
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
];
const DE: &[(&str, &str)] = &[
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
    ("View the next label", "Nächstes Etikett anzeigen"),
    (
        "View the original document and adjust the crop",
        "Originaldokument ansehen und Zuschnitt anpassen",
    ),
    ("View the previous label", "Vorheriges Etikett anzeigen"),
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
];
const FR: &[(&str, &str)] = &[
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
    ("View the next label", "Afficher l'étiquette suivante"),
    (
        "View the original document and adjust the crop",
        "Voir le document d'origine et ajuster le recadrage",
    ),
    ("View the previous label", "Afficher l'étiquette précédente"),
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
}
