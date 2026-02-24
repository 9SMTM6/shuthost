use axum::http::HeaderName;
use axum::{
    body::Body,
    http::{HeaderValue, Request},
    middleware::Next,
};
use hyper::StatusCode;

/// Custom failure handling for the trace layer. 503 responses are logged
/// at `INFO` instead of `ERROR` so they don't fill the error log.
#[derive(Clone, Copy)]
pub(crate) struct LevelAdjustingOnFailure;

impl tower_http::trace::OnFailure<tower_http::classify::ServerErrorsFailureClass>
    for LevelAdjustingOnFailure
{
    fn on_failure(
        &mut self,
        failure_classification: tower_http::classify::ServerErrorsFailureClass,
        latency: core::time::Duration,
        span: &tracing::Span,
    ) {
        use tower_http::classify::ServerErrorsFailureClass as S;

        match failure_classification {
            S::StatusCode(StatusCode::SERVICE_UNAVAILABLE) => {
                tracing::info!(classification = %S::StatusCode(StatusCode::SERVICE_UNAVAILABLE), latency = %format!("{} ms", latency.as_millis()), "response failed (downgraded)");
            }
            value => {
                tower_http::trace::DefaultOnFailure::default().on_failure(value, latency, span);
            }
        }
    }
}

/// Middleware to set security headers on all responses
///
/// This is less strict than possible. It avoids using CORS, X-Frame-Options: DENY
/// and corresponding CSP attributes, since these might block some embeddings.
pub(crate) async fn secure_headers_middleware(
    req: Request<Body>,
    next: Next,
) -> axum::response::Response {
    let mut response = next.run(req).await;
    response.headers_mut().insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );

    response.headers_mut().insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(concat!(
            "default-src 'self'; ",
            "require-trusted-types-for 'script'; ",
            "script-src ",
            env!("CSP_INLINE_SCRIPTS_HASHES"),
            "; ",
            "manifest-src 'self'; ",
            "style-src-elem 'self'; ",
            "style-src-attr 'none'; ",
            "object-src 'none'; ",
            "base-uri 'none'; ",
            "frame-src 'none'; ",
            "media-src 'none'; ",
        )),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response
}
