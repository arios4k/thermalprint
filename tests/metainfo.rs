//! The app's AppStream file carries the release notes software centers show.
//! A version bump without notes would leave "No details for this release".

const METAINFO: &str = include_str!("../data/org.thermalprint.ThermalPrint.metainfo.xml");

#[test]
fn the_newest_release_entry_matches_the_crate_version_and_has_notes() {
    let first = METAINFO
        .split("<release ")
        .nth(1)
        .expect("a release entry exists");
    let version = first
        .split("version=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("the release has a version");
    assert_eq!(
        version,
        env!("CARGO_PKG_VERSION"),
        "add release notes for the new version at the top of <releases> in data/org.thermalprint.ThermalPrint.metainfo.xml"
    );
    let entry = first.split("</release>").next().unwrap();
    assert!(
        entry.contains("<description>") && entry.contains("<p>"),
        "the newest release needs a description"
    );
}

#[test]
fn release_entries_are_newest_first_and_dated() {
    let mut last = String::from("9999-99-99");
    for entry in METAINFO.split("<release ").skip(1) {
        let date = entry
            .split("date=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("every release has a date");
        assert!(
            date <= last.as_str(),
            "{date} should not be newer than {last}"
        );
        last = date.to_string();
    }
}
