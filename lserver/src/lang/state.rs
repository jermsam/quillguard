// lang/state.rs
use harper_core::{
    Dialect,
    Document,
    Lrc,
    language_detection,
    linting::{Lint, Linter, LintGroup},
    spell::FstDictionary,
};

#[derive(Clone)]
pub struct HarperConfig {
    // Shared curated dictionary instance reused across requests
    pub dictionary: Lrc<FstDictionary>,
}

impl HarperConfig {
    /// Construct a new HarperConfig with a curated dictionary.
    pub fn new() -> Self {
        let dictionary = FstDictionary::curated();
        Self { dictionary }
    }

    /// Helper: create a plain English document backed by this state's dictionary.
    pub fn create_plain_doc(&self, text: &str) -> Document {
        Document::new_plain_english(text, &self.dictionary)
    }

    /// Helper: run language detection on the given text using the shared dictionary.
    pub fn detect_language(&self, text: &str) -> bool {
        let doc = self.create_plain_doc(text);
        language_detection::is_doc_likely_english(&doc, &self.dictionary)
    }

    /// Helper: construct a curated linter and run it on the given text.
    pub fn run_lints(&self, text: &str, dialect: Dialect) -> Vec<Lint> {
        let doc = self.create_plain_doc(text);

        let mut linter = LintGroup::new_curated(self.dictionary.clone(), dialect);
        let mut lints = linter.lint(&doc);

        harper_core::remove_overlaps(&mut lints);
        lints
    }

}

// Optional, but handy for logging / debugging
impl std::fmt::Debug for HarperConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HarperConfig").finish()
    }
}
