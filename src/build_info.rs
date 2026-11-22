//! Values the publisher bakes in at build time (environment variables read by `option_env!`).
//! Each one is optional: a build without them still works, with the matching feature
//! falling back gracefully (no update checks, bring-your-own OAuth client, no website link).
//!
//! | Variable                       | Enables                                            |
//! |--------------------------------|----------------------------------------------------|
//! | `NOTCHLY_UPDATE_FEED`          | Update checks for the direct (installer) build      |
//! | `NOTCHLY_WEBSITE`              | "Website" link in Settings › About                  |
//! | `NOTCHLY_SUPPORT_URL`          | "Report a problem" link (crash notice, About)       |
//! | `NOTCHLY_GOOGLE_CLIENT_ID`     | One-click Google Calendar sign-in                   |
//! | `NOTCHLY_GOOGLE_CLIENT_SECRET` | Google desktop-client secret (not confidential)     |
//! | `NOTCHLY_MS_CLIENT_ID`         | One-click Outlook / Microsoft 365 sign-in           |

const fn nz(v: Option<&'static str>) -> Option<&'static str> {
    match v {
        Some(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

pub const UPDATE_FEED: Option<&str> = nz(option_env!("NOTCHLY_UPDATE_FEED"));
pub const WEBSITE: Option<&str> = nz(option_env!("NOTCHLY_WEBSITE"));
pub const SUPPORT_URL: Option<&str> = nz(option_env!("NOTCHLY_SUPPORT_URL"));
pub const GOOGLE_CLIENT_ID: Option<&str> = nz(option_env!("NOTCHLY_GOOGLE_CLIENT_ID"));
pub const GOOGLE_CLIENT_SECRET: Option<&str> = nz(option_env!("NOTCHLY_GOOGLE_CLIENT_SECRET"));
pub const MS_CLIENT_ID: Option<&str> = nz(option_env!("NOTCHLY_MS_CLIENT_ID"));
