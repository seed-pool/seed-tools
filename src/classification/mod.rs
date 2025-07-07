// Classification system for media categorization

pub mod engine;
pub mod mappings;
pub mod rules;

// Re-export main classification types
pub use engine::{ClassificationResult, MediaClassification, MediaClassifier};
