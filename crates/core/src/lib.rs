//! Núcleo do Terralux: modelo de cena, ruído procedural e heightfield.
pub mod assets;
pub mod mesh;
pub mod sdf;
pub mod noise;
pub mod scene;
pub mod templates;
pub mod terrain;

pub use scene::*;
pub use templates::{TemplateGroup, TemplateInfo, TEMPLATES};
pub use terrain::Heightfield;
