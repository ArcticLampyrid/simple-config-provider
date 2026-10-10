use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use axum::{
    Router,
    extract::State,
    http::{HeaderValue, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use percent_encoding::percent_decode_str;
use url::Url;

use crate::{
    acl::is_allowed,
    config,
    error::Error,
    formats::Format,
    provider::{self, normalize_flavor_name},
};

pub fn router(config_dir: PathBuf) -> Router {
    Router::new().fallback(handle).with_state(config_dir)
}

#[derive(Debug, thiserror::Error)]
enum Failure {
    #[error("{1}")]
    Rejected(StatusCode, &'static str),
    #[error(transparent)]
    Provider(#[from] Error),
}

impl Failure {
    fn status(&self) -> StatusCode {
        match self {
            Self::Rejected(status, _) => *status,
            Self::Provider(Error::FlavorNotFound(_)) => StatusCode::NOT_FOUND,
            Self::Provider(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

struct Target {
    flavor: String,
    format: Format,
    token: String,
}

fn bad_request(message: &'static str) -> Failure {
    Failure::Rejected(StatusCode::BAD_REQUEST, message)
}

fn has_valid_escapes(path: &str) -> bool {
    path.split('%').skip(1).all(|rest| {
        rest.as_bytes()
            .get(..2)
            .is_some_and(|pair| pair.iter().all(u8::is_ascii_hexdigit))
    })
}

fn parse_target(uri: &Uri) -> Result<Target, Failure> {
    // Resolve the request like a browser would, including dot segments.
    let url = Url::parse("http://localhost")
        .and_then(|base| base.join(&uri.to_string()))
        .map_err(|_| bad_request("Invalid request URL"))?;
    if !has_valid_escapes(url.path()) {
        return Err(bad_request("Invalid URL encoding"));
    }
    let path = percent_decode_str(url.path())
        .decode_utf8()
        .map_err(|_| bad_request("Invalid URL encoding"))?;
    let path = path
        .strip_prefix('/')
        .filter(|path| !path.contains('\\'))
        .ok_or_else(|| bad_request("Invalid request path"))?;
    let extension = Path::new(path)
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default();
    let format = Format::from_extension(extension).ok_or(Failure::Rejected(
        StatusCode::NOT_FOUND,
        "The URL must end in a supported output format",
    ))?;
    let flavor = &path[..path.len() - extension.len() - 1];
    // Only canonical names are served, so ACL filters see the flavor that is loaded.
    if normalize_flavor_name(flavor).ok().as_deref() != Some(flavor) {
        return Err(bad_request("Invalid flavor path"));
    }
    let token = url
        .query_pairs()
        .find_map(|(key, value)| (key == "token").then(|| value.into_owned()))
        .unwrap_or_default();
    Ok(Target {
        flavor: flavor.into(),
        format,
        token,
    })
}

async fn serve(config_dir: PathBuf, uri: &Uri) -> Result<(String, Format), Failure> {
    let Target {
        flavor,
        format,
        token,
    } = parse_target(uri)?;
    let config = config::load(&config_dir).await?;
    if !is_allowed(&config.acls, &token, &flavor) {
        return Err(if token.is_empty() {
            Failure::Rejected(StatusCode::UNAUTHORIZED, "Authentication required")
        } else {
            Failure::Rejected(StatusCode::FORBIDDEN, "Forbidden")
        });
    }
    let body = provider::render(config_dir, flavor, format, config.vm_options).await?;
    Ok((body, format))
}

fn error_response(failure: &Failure) -> Response {
    let status = failure.status();
    let message = if status.is_server_error() {
        "Internal server error".into()
    } else {
        failure.to_string()
    };
    let body = serde_json::json!({ "error": message }).to_string();
    let mut response = (
        status,
        [(header::CONTENT_TYPE, Format::Json.content_type())],
        body,
    )
        .into_response();
    if status == StatusCode::METHOD_NOT_ALLOWED {
        response
            .headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("GET, HEAD"));
    }
    response
}

// Axum's routing layer sets Content-Length and strips HEAD response bodies.
async fn handle(State(config_dir): State<PathBuf>, method: Method, uri: Uri) -> Response {
    let result = if method == Method::GET || method == Method::HEAD {
        serve(config_dir, &uri).await
    } else {
        Err(Failure::Rejected(
            StatusCode::METHOD_NOT_ALLOWED,
            "Method not allowed",
        ))
    };
    match result {
        Ok((body, format)) => {
            ([(header::CONTENT_TYPE, format.content_type())], body).into_response()
        }
        Err(failure) => {
            tracing::warn!(status = %failure.status(), %method, path = uri.path(), error = %failure, "request failed");
            error_response(&failure)
        }
    }
}
