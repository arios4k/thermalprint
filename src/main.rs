use thermalprint::ui::ThermalPrint;
fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "thermalprint=info".into()),
        )
        .init();
    let icon = image::load_from_memory(include_bytes!("../data/org.thermalprint.ThermalPrint.png"))
        .expect("bundled icon must decode")
        .to_rgba8();
    let icon = image::imageops::resize(&icon, 64, 64, image::imageops::FilterType::Lanczos3);
    let icon = eframe::egui::IconData {
        rgba: icon.into_raw(),
        width: 64,
        height: 64,
    };
    // Match snapd's exported desktop filename for Wayland and X11 grouping.
    let app_id = if std::env::var("SNAP_NAME").is_ok_and(|name| name == "thermalprint") {
        format!(
            "{}_thermalprint",
            std::env::var("SNAP_INSTANCE_NAME").unwrap_or_else(|_| "thermalprint".into())
        )
    } else {
        "org.thermalprint.ThermalPrint".into()
    };
    let options = eframe::NativeOptions {
        persist_window: std::env::var_os("THERMALPRINT_SCREENSHOTS").is_none(),
        viewport: eframe::egui::ViewportBuilder::default()
            .with_app_id(app_id)
            .with_decorations(false)
            .with_inner_size([1240.0, 840.0])
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
