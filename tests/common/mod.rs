use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use simple_config_provider::{config, formats::Format, provider, server};
use std::fs;
use tempfile::TempDir;
use tower::ServiceExt;

pub struct Fixture(pub TempDir);

impl Fixture {
    pub fn new() -> Self {
        let fixture = Self(tempfile::tempdir().unwrap());
        for (path, text) in [
            (
                "config.json5",
                "{acls:[{token:'secret',filter:['^base$',{pattern:'^_',invert:true}]},{token:'',filter:'^public$'},{token:'admin'}]}",
            ),
            ("flavor/base.json", r#"{"name":"base","list":[1,2]}"#),
            ("flavor/public.yaml", "message: hello\n"),
            ("flavor/nested/deep.toml", "title = 'deep'"),
            ("flavor/scalar.json5", "42"),
            ("flavor/empty.json", ""),
            ("flavor/_hidden.json", r#"{"hidden":true}"#),
            ("flavor/prio.mjs", "export default () => 'script'"),
            ("flavor/prio.js", "export default () => 'js'"),
            ("flavor/prio.json", "'plain'"),
            (
                "flavor/pc.mjs",
                "export default async input => ({...await applyFlavor(input, 'base'), platform:'pc'})",
            ),
            ("flavor/echo.mjs", "export default input => ({input})"),
            (
                "flavor/chain.mjs",
                "export default input => applyFlavor(input, ['base','echo'])",
            ),
            ("flavor/broken.mjs", "export const notDefault = 1"),
            (
                "flavor/throws.mjs",
                "export default () => {throw new Error('upstream flavor not found')}",
            ),
            (
                "flavor/missing-nested.mjs",
                "export default input => applyFlavor(input, 'nope')",
            ),
        ] {
            fixture.write(path, text);
        }
        fixture
    }

    pub fn write(&self, path: &str, text: &str) {
        let path = self.0.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    pub fn router(&self) -> Router {
        server::router(self.0.path().to_owned())
    }

    pub async fn render(&self, name: &str) -> Value {
        let path = self.0.path().to_owned();
        let config = config::load(&path).await.unwrap();
        let body = provider::render(path, name.into(), Format::Json, config.vm_options)
            .await
            .unwrap();
        serde_json::from_str(&body).unwrap()
    }
}

pub async fn request(
    app: Router,
    method: &str,
    path: &str,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8(body.to_vec()).unwrap())
}
