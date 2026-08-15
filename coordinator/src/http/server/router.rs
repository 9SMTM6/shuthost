use core::time::Duration;

use axum::{
    Router,
    extract::State,
    http::{
        Method, StatusCode,
        header::{AUTHORIZATION, COOKIE},
    },
    middleware::{self as ax_middleware},
    response::{IntoResponse as _, Response},
    routing::{IntoMakeService, any, get},
};
use tower::ServiceBuilder;
use tower_http::{
    ServiceBuilderExt as _, request_id::MakeRequestUuid, timeout::TimeoutLayer, trace::TraceLayer,
};

use crate::{
    app::AppState,
    http::{
        api, assets, auth, download, login, m2m,
        middleware::{LevelAdjustingOnFailure, secure_headers_middleware},
        push,
    },
    websocket,
};

/// Creates the main application router by merging public and private routes.
///
/// Public routes include authentication endpoints (login, logout, OIDC), static assets,
/// downloads, and M2M APIs that are accessible without authentication.
/// Private routes include the main UI, API endpoints, and WebSocket handler, protected by auth middleware.
///
/// When routes get added to public routes, [`crate::http::server::EXPECTED_AUTH_EXCEPTIONS_VERSION`] needs to be bumped.
///
/// Note: auth middleware is NOT applied here; it is added in [`create_app`] where `AppState` is available.
pub(crate) fn create_app_router(
    spa_handler: impl Fn(AppState) -> Response + Send + Sync + Clone + 'static,
) -> Router<AppState> {
    let public = Router::new()
        .merge(login::routes())
        .merge(assets::routes())
        .nest("/download", download::routes())
        .nest("/api/m2m", m2m::routes());

    let private = Router::new()
        .nest("/api", api::routes())
        .nest("/api/push", push::routes())
        .route(
            "/",
            get({
                let spa_handler = spa_handler.clone();
                async move |State(state): State<AppState>| spa_handler(state)
            }),
        )
        .route("/ws", any(websocket::ws_handler));

    public
        .merge(private)
        // Any unmatched /api/* path gets a clean 404; this must be registered
        // before the fallback so it is matched with higher precedence.
        .route("/api/{*path}", any(|| async { StatusCode::NOT_FOUND }))
        .fallback(async move |method: Method, State(state): State<AppState>| {
            // Fallback handler for unmatched routes: serves the SPA shell for GET/HEAD
            // requests (letting the client-side router render the correct page, including
            // the 404 page), and returns 404 for all other methods.
            if method == Method::GET || method == Method::HEAD {
                spa_handler(state)
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        })
}

pub(crate) fn create_app(app_state: AppState) -> IntoMakeService<Router<()>> {
    #[expect(clippy::absolute_paths, reason = "I dont want conditional imports")]
    let middleware_stack = ServiceBuilder::new()
        .sensitive_headers([AUTHORIZATION, COOKIE])
        .set_x_request_id(MakeRequestUuid)
        .propagate_x_request_id()
        .layer(TraceLayer::new_for_http().on_failure(LevelAdjustingOnFailure))
        .layer(cfg_if_expr!(
            #[cfg(any(
                feature = "compression-br",
                feature = "compression-deflate",
                feature = "compression-gzip",
                feature = "compression-zstd",
            ))]
            tower_http::compression::CompressionLayer::new(),
            #[cfg(not)]
            tower::layer::util::Identity::new(),
        ))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(ax_middleware::from_fn(secure_headers_middleware));

    let app = create_app_router(assets::serve_ui)
        // Apply auth middleware using AppState directly so the middleware
        // has access to both auth runtime and config_rx for HMAC validation.
        .route_layer(ax_middleware::from_fn_with_state(
            app_state.clone(),
            auth::require,
        ))
        .with_state(app_state)
        .layer(middleware_stack);

    app.into_make_service()
}
