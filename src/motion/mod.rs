//! Motion: springs and named choreography constants.

pub mod spring;

pub use spring::*;

/// Named choreography timings (section 5). Kept in one place so every
/// transition behaves the same everywhere.
pub mod choreo {
    /// Height starts this long after width on expand.
    pub const HEIGHT_DELAY_MS: f32 = 20.0;
    /// Old content fades out over this long when the island changes state.
    pub const CONTENT_OUT_MS: f32 = 80.0;
    /// Content fades out before a collapse.
    pub const COLLAPSE_OUT_MS: f32 = 90.0;
    /// New content starts when shape progress reaches this.
    pub const REVEAL_AT: f32 = 0.55;
    /// Live content fades in at this progress when collapsing to Live.
    pub const LIVE_REVEAL_AT: f32 = 0.60;
    /// Stagger between rows.
    pub const STAGGER_MS: f32 = 25.0;
    /// Entering rows rise from this offset.
    pub const RISE_DIP: f32 = 6.0;
    pub const SCALE_FROM: f32 = 0.97;
    /// Page switch slide distance.
    pub const PAGE_SLIDE_DIP: f32 = 28.0;
    /// Activity swap roll distance.
    pub const ROLL_DIP: f32 = 10.0;
    /// Pop-up exit fade.
    pub const POPUP_OUT_MS: f32 = 100.0;
    /// Reduced-motion fade cap.
    pub const REDUCED_FADE_MS: f32 = 120.0;
}
