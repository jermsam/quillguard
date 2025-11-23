// lang/mod.rs
pub mod state;
pub mod lint;
pub mod grammar;

pub use state::HarperConfig;
pub use lint::JSONSuggestion;
pub use grammar::{GrammarCorrector, Corrector};
