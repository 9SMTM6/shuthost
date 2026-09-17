//! Authentication info inserted by the auth middleware.

/// Information about the authenticated identity for a request.
#[derive(Debug, Clone)]
pub(crate) enum AuthInfo {
    /// Authenticated via cookie (Token or OIDC session), i.e. a browser user.
    WebSession,
    /// Authenticated via HMAC headers (M2M client).
    M2MClient { client_id: String },
}
