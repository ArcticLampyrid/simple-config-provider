mod common;

use axum::{Router, http::StatusCode};
use common::{Fixture, request};
use serde_json::{Value, json};
use simple_config_provider::{config, formats::Format, provider};

#[tokio::test]
async fn plain_flavors_and_formats() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.render("base").await,
        json!({"name":"base","list":[1,2]})
    );
    assert_eq!(fixture.render("public").await, json!({"message":"hello"}));
    assert_eq!(fixture.render("nested/deep").await, json!({"title":"deep"}));
    assert_eq!(fixture.render("scalar").await, json!(42));
    for extension in ["json", "JSON5", "yaml", "yml", "toml"] {
        let (status, headers, body) = request(
            fixture.router(),
            "GET",
            &format!("/base.{extension}?token=secret"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let format = Format::from_extension(extension).unwrap();
        assert_eq!(headers["content-type"], format.content_type());
        let value = simple_config_provider::formats::parse(&body, format).unwrap();
        assert_eq!(value["name"].as_str(), Some("base"));
    }
}

#[tokio::test]
async fn scripts_and_chains() {
    let fixture = Fixture::new();
    assert_eq!(fixture.render("prio").await, json!("script"));
    assert_eq!(
        fixture.render("pc").await,
        json!({"name":"base","list":[1,2],"platform":"pc"})
    );
    assert_eq!(
        fixture.render("chain").await,
        json!({"input":{"name":"base","list":[1,2]}})
    );
    assert_eq!(fixture.render("echo").await, json!({}));
    // Every output format shares the serde conversion of the script result.
    fixture.write("flavor/undefined.mjs", "export default () => undefined");
    assert_eq!(fixture.render("undefined").await, json!(null));
    fixture.write(
        "flavor/js-values.mjs",
        "export default () => ({ drop: undefined, nan: NaN, date: new Date(0), fn: () => 1 })",
    );
    assert_eq!(
        fixture.render("js-values").await,
        json!({"nan":null,"date":"1970-01-01T00:00:00.000Z"})
    );
    for extension in ["json", "json5", "yaml", "toml"] {
        let (status, _, body) = request(
            fixture.router(),
            "GET",
            &format!("/pc.{extension}?token=secret"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{extension}: {body}");
        let value = simple_config_provider::formats::parse(
            &body,
            Format::from_extension(extension).unwrap(),
        )
        .unwrap();
        assert_eq!(value["platform"].as_str(), Some("pc"), "{extension}");
        assert_eq!(value["list"][1].as_i64(), Some(2), "{extension}: {body}");
    }
}

#[tokio::test]
async fn authentication_methods_and_typed_errors() {
    let fixture = Fixture::new();
    for (path, expected, message) in [
        ("/base.json", 401, "Authentication required"),
        ("/base.json?token=wrong", 403, "Forbidden"),
        ("/_hidden.json?token=secret", 403, "Forbidden"),
        ("/missing.json?token=wrong", 403, "Forbidden"),
        (
            "/missing.json?token=admin",
            404,
            "Flavor not found: missing",
        ),
        (
            "/base.txt?token=admin",
            404,
            "The URL must end in a supported output format",
        ),
        (
            "/.json?token=admin",
            404,
            "The URL must end in a supported output format",
        ),
        (
            "/nested%2F..%2Fbase.json?token=admin",
            400,
            "Invalid flavor path",
        ),
        ("/a//b.json?token=admin", 400, "Invalid flavor path"),
        ("/a%00b.json?token=admin", 400, "Invalid flavor path"),
        ("/%E0%A4%A.json?token=admin", 400, "Invalid URL encoding"),
        ("/%FF.json?token=admin", 400, "Invalid URL encoding"),
        ("/a%5Cb.json?token=admin", 400, "Invalid request path"),
        ("/throws.json?token=admin", 500, "Internal server error"),
        (
            "/missing-nested.json?token=admin",
            500,
            "Internal server error",
        ),
        ("/broken.json?token=admin", 500, "Internal server error"),
        ("/scalar.toml?token=admin", 500, "Internal server error"),
    ] {
        let (status, _, body) = request(fixture.router(), "GET", path).await;
        assert_eq!(status.as_u16(), expected, "{path}: {body}");
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap(),
            json!({"error": message}),
            "{path}"
        );
    }
    let (status, headers, _) = request(fixture.router(), "POST", "/base.json?token=admin").await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(headers["allow"], "GET, HEAD");
    for path in [
        "/public.json",
        "/_hidden.json?token=admin",
        "/base.json?token=admin&token=wrong",
    ] {
        assert_eq!(
            request(fixture.router(), "GET", path).await.0,
            StatusCode::OK
        );
    }
    assert_eq!(
        request(
            fixture.router(),
            "GET",
            "/base.json?token=wrong&token=admin"
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn head_keeps_get_length_and_does_not_send_a_body() {
    let fixture = Fixture::new();
    fixture.write("flavor/unicode.json", r#"{"text":"你好"}"#);
    for path in [
        "/unicode.json?token=admin",
        "/pc.yaml?token=secret",
        "/missing.json?token=admin",
    ] {
        let (get_status, get_headers, get_body) = request(fixture.router(), "GET", path).await;
        let (head_status, head_headers, head_body) = request(fixture.router(), "HEAD", path).await;
        assert_eq!(get_status, head_status);
        assert_eq!(
            head_headers["content-length"],
            get_headers["content-length"]
        );
        assert_eq!(get_headers["content-length"], get_body.len().to_string());
        assert!(head_body.is_empty());
    }
}

#[tokio::test]
async fn values_are_isolated_and_modules_are_request_local() {
    let fixture = Fixture::new();
    fixture.write(
        "flavor/mutate.js",
        "export default input => { input.list.push(3); return input }",
    );
    fixture.write(
        "flavor/isolation.mjs",
        r#"
        let calls = 0;
        export default async () => {
            const original = await applyFlavor(null, 'base');
            const changed = await applyFlavor(original, 'mutate');
            const unchanged = await applyFlavor(null, 'base');
            const empty = await applyFlavor(original, []);
            empty.list.push(4);
            return { original, changed, unchanged, calls: ++calls };
        }
    "#,
    );
    let expected = json!({
        "original":{"name":"base","list":[1,2]},
        "changed":{"name":"base","list":[1,2,3]},
        "unchanged":{"name":"base","list":[1,2]},"calls":1
    });
    for _ in 0..10 {
        assert_eq!(fixture.render("isolation").await, expected);
    }
}

#[tokio::test]
async fn nested_async_and_parallel_applications() {
    let fixture = Fixture::new();
    fixture.write(
        "flavor/async-base.mjs",
        r#"
        export default async input => {
            await Promise.resolve();
            await new Promise(resolve => setTimeout(resolve, 1));
            return applyFlavor(input, 'base');
        }
    "#,
    );
    fixture.write(
        "flavor/branch-a.mjs",
        "export default input => applyFlavor(input, 'async-base')",
    );
    fixture.write(
        "flavor/branch-b.mjs",
        "export default input => applyFlavor(input, 'branch-a')",
    );
    fixture.write(
        "flavor/parallel.mjs",
        r#"
        export default async input => {
            const result = await Promise.all([
                applyFlavor(input, 'branch-a'), applyFlavor(input, 'branch-b'),
                new Promise((resolve, reject) => setTimeout(() => applyFlavor(input, 'async-base').then(resolve,reject),1)),
            ]);
            return result;
        }
    "#,
    );
    let base = json!({"name":"base","list":[1,2]});
    assert_eq!(fixture.render("parallel").await, json!([base, base, base]));
}

#[tokio::test]
async fn builtin_modules_and_relative_esm_imports() {
    let fixture = Fixture::new();
    fixture.write("flavor/helper.js", "export const label = 'helper'");
    fixture.write(
        "flavor/modules.mjs",
        r#"
        import { Buffer as ImportedBuffer } from 'node:buffer';
        import { Buffer as BareBuffer } from 'buffer';
        import { URL, URLSearchParams } from 'node:url';
        import { label } from './helper.js';
        export default async () => {
            const text = '你好 / hello';
            const encoded = ImportedBuffer.from(text).toString('base64');
            return {
                label, same: ImportedBuffer === Buffer && Buffer === BareBuffer,
                text: Buffer.from(encoded, 'base64').toString('utf8'),
                base64: btoa('hello'), decoded: atob('aGVsbG8='),
                query: new URL('https://example.com/?q=value').searchParams.get('q'),
                params: new URLSearchParams({a:'x y'}).toString(),
                utf8: new TextDecoder().decode(new TextEncoder().encode(text)),
                json: JSON.parse(JSON.stringify({answer:42})),
            };
        }
    "#,
    );
    assert_eq!(
        fixture.render("modules").await,
        json!({"label":"helper","same":true,"text":"你好 / hello","base64":"aGVsbG8=","decoded":"hello","query":"value","params":"a=x+y","utf8":"你好 / hello","json":{"answer":42}})
    );
}

#[tokio::test]
async fn files_acl_and_imported_modules_reload_without_metadata_cache() {
    let fixture = Fixture::new();
    fixture.write("flavor/hot.json", "1");
    fixture.write("flavor/helper.js", "export const value = 1");
    fixture.write(
        "flavor/hot-script.mjs",
        "import {value} from './helper.js'; export default () => value",
    );
    let app = fixture.router();
    assert_eq!(fixture.render("hot").await, json!(1));
    assert_eq!(fixture.render("hot-script").await, json!(1));
    fixture.write("flavor/hot.json", "2");
    fixture.write("flavor/helper.js", "export const value = 2");
    assert_eq!(fixture.render("hot").await, json!(2));
    assert_eq!(fixture.render("hot-script").await, json!(2));
    fixture.write("config.json5", "{acls:{new:['base']}}");
    assert_eq!(
        request(app.clone(), "GET", "/base.json?token=admin")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(app.clone(), "GET", "/base.json?token=new").await.0,
        StatusCode::OK
    );
    fixture.write("config.json5", "not valid json5");
    assert_eq!(
        request(app, "GET", "/base.json?token=new").await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn llrt_vm_provides_globals_native_modules_and_embedded_streams() {
    let fixture = Fixture::new();
    fixture.write(
        "flavor/vm.mjs",
        r#"
        import { Buffer } from 'node:buffer';
        import { basename } from 'node:path';
        import { setTimeout } from 'node:timers';
        import { Readable } from 'node:stream';
        import { finished } from 'node:stream/promises';
        export default async () => {
            await new Promise(resolve => setTimeout(resolve, 1));
            return {
                globals: global === globalThis && self === globalThis,
                process: typeof process.version,
                console: typeof console.log,
                clone: typeof structuredClone,
                stream: typeof Readable,
                finished: typeof finished,
                basename: basename('/a/b.txt'),
                text: Buffer.from('aGVsbG8=', 'base64').toString(),
                crypto: crypto.getRandomValues(new Uint8Array(8)).length,
            };
        }
    "#,
    );
    assert_eq!(
        fixture.render("vm").await,
        json!({
            "globals":true,"process":"string","console":"function","clone":"function",
            "stream":"function","finished":"function","basename":"b.txt","text":"hello","crypto":8,
        })
    );
}

#[tokio::test]
async fn llrt_structured_clone_handles_circular_and_typed_values() {
    let fixture = Fixture::new();
    fixture.write("flavor/clone.mjs", r#"
        export default () => {
            const original = { list:[1,2], date:new Date(0), map:new Map([['key',42]]), bytes:new Uint8Array([1,2]) };
            original.self = original;
            const cloned = structuredClone(original);
            cloned.list.push(3);
            cloned.bytes[0] = 9;
            return {
                distinct: cloned !== original,
                cycle: cloned.self === cloned,
                original: original.list,
                cloned: cloned.list,
                date: cloned.date instanceof Date && cloned.date.getTime() === 0,
                map: cloned.map instanceof Map && cloned.map.get('key') === 42,
                bytes: [original.bytes[0], cloned.bytes[0]],
            };
        }
    "#);
    assert_eq!(
        fixture.render("clone").await,
        json!({
            "distinct":true,"cycle":true,"original":[1,2],"cloned":[1,2,3],"date":true,"map":true,"bytes":[1,9],
        })
    );
}

#[tokio::test]
async fn script_failures_do_not_terminate_the_service() {
    let fixture = Fixture::new();
    fixture.write(
        "flavor/reject.mjs",
        "export default async () => { await Promise.resolve(); throw new Error('failure') }",
    );
    fixture.write("flavor/syntax.mjs", "export default () => {");
    fixture.write(
        "flavor/import-missing.mjs",
        "import './absent.mjs'; export default () => 1",
    );
    let app = fixture.router();
    for name in ["reject", "syntax", "import-missing"] {
        let (status, _, body) =
            request(app.clone(), "GET", &format!("/{name}.json?token=admin")).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{name}: {body}");
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap(),
            json!({"error":"Internal server error"})
        );
        assert_eq!(
            request(app.clone(), "GET", "/pc.json?token=admin").await.0,
            StatusCode::OK
        );
    }
}

#[tokio::test]
async fn fetch_json_text_bytes_and_http_errors() {
    use axum::{response::IntoResponse, routing::get};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let upstream = tokio::spawn(async move {
        let app = Router::new()
            .route(
                "/json",
                get(|| async {
                    (
                        [("content-type", "application/json")],
                        r#"{"value":"remote"}"#,
                    )
                }),
            )
            .route(
                "/error",
                get(|| async { (StatusCode::NOT_FOUND, "missing").into_response() }),
            );
        axum::serve(listener, app).await.unwrap();
    });
    let fixture = Fixture::new();
    fixture.write("flavor/fetch.mjs", &format!(r#"
        export default async input => {{
            const response = await fetch('http://{address}/json');
            const data = await response.json();
            const base = await applyFlavor(input, 'base');
            const missing = await fetch('http://{address}/error');
            const bytes = await (await fetch('http://{address}/json')).arrayBuffer();
            return {{data, base, status:missing.status, ok:missing.ok, text:await missing.text(), bytes:Buffer.from(bytes).toString()}};
        }}
    "#));
    let result = fixture.render("fetch").await;
    upstream.abort();
    assert_eq!(
        result,
        json!({"data":{"value":"remote"},"base":{"name":"base","list":[1,2]},"status":404,"ok":false,"text":"missing","bytes":r#"{"value":"remote"}"#})
    );
}

#[tokio::test]
async fn vm_limits_stop_runaway_flavors() {
    let fixture = Fixture::new();
    fixture.write(
        "config.json5",
        "{acls:{admin:null}, vm_options:{memory_limit:'8MiB', max_stack_size:'256KiB'}}",
    );
    fixture.write(
        "flavor/self.mjs",
        "export default i => applyFlavor(i, 'self')",
    );
    fixture.write(
        "flavor/deep.mjs",
        "export default () => { const f = n => f(n + 1) + 1; return f(0) }",
    );
    let config = config::load(fixture.0.path()).await.unwrap();
    for (name, message) in [
        ("self", "out of memory"),
        ("deep", "Maximum call stack size exceeded"),
    ] {
        let error = provider::render(
            fixture.0.path().to_owned(),
            name.into(),
            Format::Json,
            config.vm_options,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains(message), "{name}: {error}");
        assert_eq!(
            request(
                fixture.router(),
                "GET",
                &format!("/{name}.json?token=admin")
            )
            .await
            .0,
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
    assert_eq!(
        request(fixture.router(), "GET", "/pc.json?token=admin")
            .await
            .0,
        StatusCode::OK
    );
}
