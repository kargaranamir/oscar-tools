//! Operation traits
//!
//! A subset of these should be implemented for different corpus versions.
mod checksum;
mod compress;
mod extract_text;
mod split;
pub(crate) use checksum::Checksum;
pub(crate) use compress::Compress;
pub(crate) use extract_text::ExtractText;
pub(crate) use split::Split;
