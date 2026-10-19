//! Mail transport integration: MIME construction, submission to the relay,
//! and return-path/bounce handling.

pub mod bounce;
pub mod mime;
pub mod smtp;
