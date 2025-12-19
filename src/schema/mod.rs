mod annotation;
mod parser;
mod refs;

pub use annotation::{Annotation, AnnotationMap};
pub use parser::extract_annotations;
pub use refs::resolve_refs;
