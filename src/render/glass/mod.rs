//! Liquid Glass: the material system behind the notch.
//!
//! * [`backend`] picks the tier (own-rendered Liquid, system blur, solid) and says why.
//! * [`duplication`] reads the pixels behind the notch on a worker thread (Liquid tier).
//! * [`lens`] builds the edge-lens displacement map from the shape's signed distance.
//! * [`compositor`] draws glass and glass-on-glass tiles with Direct2D effects.
//! * [`vibrancy`] keeps text legible over whatever is behind.

pub mod backend;
pub mod compositor;
pub mod duplication;
pub mod lens;
pub mod vibrancy;

pub use backend::Tier;
pub use compositor::{Backdrop, GlassFrame, GlassLayer, GlassRes};
