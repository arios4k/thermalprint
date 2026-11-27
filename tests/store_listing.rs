//! The Microsoft Store listing that the release workflow submits. A version bump without new
//! "What's new" text, or search terms the Store would reject, fails here instead of in Partner Center.
use serde_json::Value;
use std::collections::HashSet;

const LISTING: &str = include_str!("../packaging/windows/store-listing.json");
const LANGUAGES: [&str; 4] = ["en-us", "es", "de", "fr"];

fn listing() -> Value {
    serde_json::from_str(LISTING).expect("store-listing.json is valid JSON")
}

#[test]
fn the_listing_matches_the_crate_version() {
    let listing = listing();
    assert_eq!(
        listing["version"],
        env!("CARGO_PKG_VERSION"),
        "update packaging/windows/store-listing.json (version and releaseNotes) for the new version"
    );
    for language in LANGUAGES {
        let notes = listing["listings"][language]["releaseNotes"]
            .as_str()
            .unwrap_or_else(|| panic!("{language} needs releaseNotes"));
        assert!(
            notes.contains(env!("CARGO_PKG_VERSION")),
            "the {language} release notes should name version {}",
            env!("CARGO_PKG_VERSION")
        );
        assert!(
            notes.chars().count() <= 1500,
            "{language} release notes are over 1,500 characters"
        );
    }
}

#[test]
fn every_package_language_has_a_listing_and_nothing_else() {
    let listing = listing();
    let mut keys: Vec<&str> = listing["listings"]
        .as_object()
        .expect("listings is an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    let mut expected = LANGUAGES.to_vec();
    expected.sort_unstable();
    assert_eq!(keys, expected);
}

#[test]
fn search_terms_fit_the_store_rules_and_never_repeat_a_word() {
    let listing = listing();
    for language in LANGUAGES {
        let terms: Vec<&str> = listing["listings"][language]["searchTerms"]
            .as_array()
            .unwrap_or_else(|| panic!("{language} needs searchTerms"))
            .iter()
            .map(|t| t.as_str().expect("search terms are strings"))
            .collect();
        assert!(
            (1..=7).contains(&terms.len()),
            "{language}: the Store allows up to 7 search terms"
        );
        let mut seen = HashSet::new();
        let mut words = 0;
        for term in &terms {
            assert!(
                term.chars().count() <= 30,
                "{language}: \"{term}\" is over 30 characters"
            );
            for word in term.split_whitespace() {
                words += 1;
                let word = word.to_lowercase();
                assert!(
                    !word.contains("thermalprint"),
                    "{language}: the app name is already searchable"
                );
                assert!(
                    seen.insert(word.clone()),
                    "{language}: \"{word}\" is repeated"
                );
            }
        }
        assert!(
            words <= 21,
            "{language}: {words} words, the Store allows 21"
        );
        let short = listing["listings"][language]["shortDescription"]
            .as_str()
            .unwrap_or_else(|| panic!("{language} needs a shortDescription"));
        assert!(!short.is_empty() && short.chars().count() <= 1000);
    }
}

#[test]
fn pricing_names_a_price_and_a_trial_the_store_accepts() {
    let listing = listing();
    let price = listing["pricing"]["priceId"]
        .as_str()
        .expect("pricing.priceId is set");
    assert!(
        price == "Free"
            || price
                .strip_prefix("Tier")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())),
        "pricing.priceId must be Free or a Store tier such as Tier1052 ($4.99), not {price}"
    );
    if price == "Free" {
        assert_eq!(
            listing["pricing"]["trialPeriod"], "NoFreeTrial",
            "a free app has no trial"
        );
    }
    let trial = listing["pricing"]["trialPeriod"]
        .as_str()
        .expect("pricing.trialPeriod is set");
    assert!(
        [
            "NoFreeTrial",
            "OneDay",
            "SevenDays",
            "FifteenDays",
            "ThirtyDays",
            "TrialNeverExpires"
        ]
        .contains(&trial),
        "pricing.trialPeriod {trial} is not a Store trial value"
    );
}
