use thermalprint::ui::ThermalPrint;
fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "thermalprint=info".into()),
        )
        .init();
    let icon = eframe::icon_data::from_png_bytes(include_bytes!(
        "../data/org.thermalprint.ThermalPrint.png"
    ))
    .expect("bundled icon must decode");
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 820.0])
            .with_min_inner_size([760.0, 560.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "ThermalPrint",
        options,
        Box::new(|cc| Ok(Box::new(ThermalPrint::new(cc)))),
    )
}
