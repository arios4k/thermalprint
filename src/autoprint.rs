//! The rules for printing a label automatically when it lands in the watch folder. They are
//! kept apart from the screen so they can be tested: which labels may print on their own,
//! which file names are allowed, and how many may print per minute.
use std::{
    collections::VecDeque,
    path::Path,
    time::{Duration, Instant},
};

/// How long a label waits, with a Cancel button, before it prints.
pub const COUNTDOWN: Duration = Duration::from_secs(5);
/// The default most labels printed automatically in one minute.
pub const DEFAULT_PER_MINUTE: u8 = 10;
pub const MAX_PER_MINUTE: u8 = 60;
/// The longest file-name filter that is kept.
pub const MAX_PATTERN_CHARS: usize = 200;

/// Does `text` match a pattern that may use `*` (any run of letters) and `?` (one letter)?
/// Capital letters and small letters count as the same.
fn glob_matches(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().flat_map(char::to_lowercase).collect();
    let text: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let (mut p, mut t) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            mark = t;
            p += 1;
        } else if let Some(s) = star {
            p = s + 1;
            mark += 1;
            t = mark;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

/// Is a file name allowed by the filter? The filter is a list of patterns separated by commas
/// or semicolons, for example `label*.pdf, order-?.png`. An empty filter allows every name.
pub fn name_matches(filter: &str, name: &str) -> bool {
    let patterns: Vec<&str> = filter
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    patterns.is_empty() || patterns.iter().any(|p| glob_matches(p, name))
}

/// A label that has just arrived in the watch folder.
pub struct Arrival<'a> {
    pub name: &'a str,
    /// The crop detection was not confident, so a person should look at it first.
    pub needs_review: bool,
    pub pages: usize,
    pub already_printed: bool,
}

/// Why a label is left in the queue instead of printing by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    AlreadyPrinted,
    NameFilter,
    MultiPage,
    NeedsReview,
}
impl Skip {
    /// What to tell the user, or `None` when staying quiet is better: a name the filter
    /// leaves out, or a file that already printed, is not news.
    pub fn message(self) -> Option<&'static str> {
        match self {
            Skip::AlreadyPrinted | Skip::NameFilter => None,
            Skip::MultiPage => Some(
                "has several pages, so it was not printed automatically. Review it in the queue.",
            ),
            Skip::NeedsReview => Some(
                "needs a review, so it was not printed automatically. Check the crop in the queue.",
            ),
        }
    }
}

/// May this label print on its own? Only a label whose name passes the filter, that has a
/// single page, and whose crop needs no review.
pub fn decide(arrival: &Arrival, filter: &str) -> Result<(), Skip> {
    if arrival.already_printed {
        Err(Skip::AlreadyPrinted)
    } else if !name_matches(filter, arrival.name) {
        Err(Skip::NameFilter)
    } else if arrival.pages != 1 {
        Err(Skip::MultiPage)
    } else if arrival.needs_review {
        Err(Skip::NeedsReview)
    } else {
        Ok(())
    }
}

/// Keeps a burst of files from using up a roll: at most so many starts in any minute.
#[derive(Default)]
pub struct RateLimit {
    starts: VecDeque<Instant>,
}
impl RateLimit {
    /// Record a print start if the limit allows one now.
    pub fn allow(&mut self, now: Instant, per_minute: u8) -> bool {
        while self
            .starts
            .front()
            .is_some_and(|&t| now.duration_since(t) >= Duration::from_secs(60))
        {
            self.starts.pop_front();
        }
        if self.starts.len() < per_minute.clamp(1, MAX_PER_MINUTE) as usize {
            self.starts.push_back(now);
            true
        } else {
            false
        }
    }
}

/// Does this folder look like the Downloads folder, where invoices and other files that are
/// not labels also land?
pub fn looks_like_downloads(folder: &Path, user_downloads: Option<&Path>) -> bool {
    user_downloads.is_some_and(|d| d == folder)
        || folder
            .file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("downloads"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn filters_match_names_without_regard_to_capitals_and_allow_lists() {
        assert!(
            name_matches("", "anything.pdf"),
            "no filter allows everything"
        );
        assert!(
            name_matches("  ,; ", "anything.pdf"),
            "blank entries are ignored"
        );
        assert!(name_matches("label*.pdf", "Label-1048.PDF"));
        assert!(name_matches("order-?.png", "order-7.png"));
        assert!(!name_matches("order-?.png", "order-77.png"));
        assert!(name_matches("*.png; label*", "label.pdf"));
        assert!(!name_matches("label*.pdf", "invoice.pdf"));
        assert!(!name_matches("label*.pdf", "label.png"));
        assert!(name_matches("*", "x"));
        assert!(name_matches("a*b*c", "aXXbYYc"));
        assert!(!name_matches("a*b*c", "aXXbYY"));
    }

    #[test]
    fn only_single_page_labels_that_need_no_review_print_by_themselves() {
        let ok = Arrival {
            name: "label.pdf",
            needs_review: false,
            pages: 1,
            already_printed: false,
        };
        assert_eq!(decide(&ok, ""), Ok(()));
        let review = Arrival {
            needs_review: true,
            ..ok
        };
        assert_eq!(decide(&review, ""), Err(Skip::NeedsReview));
        let pages = Arrival { pages: 3, ..ok };
        assert_eq!(decide(&pages, ""), Err(Skip::MultiPage));
        let none = Arrival { pages: 0, ..ok };
        assert_eq!(decide(&none, ""), Err(Skip::MultiPage));
        assert_eq!(decide(&ok, "invoice*"), Err(Skip::NameFilter));
        let again = Arrival {
            already_printed: true,
            ..ok
        };
        assert_eq!(decide(&again, ""), Err(Skip::AlreadyPrinted));
    }

    #[test]
    fn quiet_skips_say_nothing_and_the_others_explain_themselves() {
        assert!(Skip::NameFilter.message().is_none());
        assert!(Skip::AlreadyPrinted.message().is_none());
        assert!(Skip::MultiPage.message().unwrap().contains("several pages"));
        assert!(
            Skip::NeedsReview
                .message()
                .unwrap()
                .contains("needs a review")
        );
    }

    #[test]
    fn the_limit_holds_a_burst_back_and_frees_up_after_a_minute() {
        let mut limit = RateLimit::default();
        let start = Instant::now();
        for _ in 0..3 {
            assert!(limit.allow(start, 3));
        }
        assert!(!limit.allow(start + Duration::from_secs(30), 3));
        assert!(limit.allow(start + Duration::from_secs(61), 3));
        // A limit of zero is treated as one, and a huge one as the maximum.
        let mut strict = RateLimit::default();
        assert!(strict.allow(start, 0));
        assert!(!strict.allow(start, 0));
    }

    #[test]
    fn the_downloads_folder_is_recognised_by_path_or_name() {
        let downloads = PathBuf::from("/home/a/Downloads");
        assert!(looks_like_downloads(&downloads, Some(&downloads)));
        assert!(looks_like_downloads(Path::new("/x/downloads"), None));
        assert!(looks_like_downloads(
            Path::new("/mnt/data/Descargas"),
            Some(Path::new("/mnt/data/Descargas"))
        ));
        assert!(!looks_like_downloads(
            Path::new("/home/a/Labels"),
            Some(&downloads)
        ));
    }
}
