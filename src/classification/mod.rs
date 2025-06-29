// Classification system for media categorization

pub mod engine;
pub mod rules;
pub mod mappings;

// Re-export main classification types
pub use engine::{MediaClassification, ClassificationResult, MediaClassifier};