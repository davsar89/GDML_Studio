# Rust Backend Stack Reference

> For GDML Studio backend development.
> Sources: [Axum docs](https://docs.rs/axum), [quick-xml](https://docs.rs/quick-xml), [serde](https://serde.rs/), [evalexpr](https://docs.rs/evalexpr), [Tokio](https://tokio.rs/)

---

## 1. Axum 0.8

### Router Setup

```rust
use axum::{Router, routing::{get, post}, extract::State};

let app = Router::new()
    .route("/users", get(list_users).post(create_user))
    .route("/users/{id}", get(get_user))
    .nest("/api", api_router)
    .with_state(app_state);
```

### Extractors

| Extractor | Purpose | Notes |
|-----------|---------|-------|
| `State<T>` | App state | `T` is the `with_state` type or derived from it via `FromRef` |
| `Json<T>` | JSON body | Consumes the body, so it must be the last extractor |
| `Path<T>` | URL path params | `Path<(String, u32)>` for multiple |
| `Query<T>` | Query string | `T: Deserialize` |
| `Multipart` | File uploads | Feature: `multipart` |

Body-consuming extractors (`Json`, `Form`, `Multipart`) must be the **last** handler argument. Only one per handler.

### Middleware

```rust
use tower_http::cors::CorsLayer;
use axum::extract::DefaultBodyLimit;

let app = Router::new()
    .route("/upload", post(upload))
    .layer(DefaultBodyLimit::max(10 * 1024 * 1024))  // 10MB
    .layer(CorsLayer::permissive());

// ServeDir for static files (nest_service at "/" panics in 0.8; use fallback_service)
app.nest_service("/assets", ServeDir::new("./assets"))
   .fallback_service(ServeDir::new("./dist"));
```

**Default body limit:** 2 MB. Set with `DefaultBodyLimit::max(bytes)`.

### Error Handling

```rust
enum AppError { NotFound, Internal(anyhow::Error) }

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found").into_response(),
            AppError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }
}
```

### Gotchas

- **0.8 path syntax:** `{param}` not `:param`
- **Nesting at the root panics in 0.8:** `nest("/", ..)` and `nest_service("/", ..)`; use `merge` or `fallback_service`
- **Layer ordering:** last `.layer()` is outermost middleware (runs first on request)

---

## 2. quick-xml

### Reader API

```rust
use quick_xml::Reader;
use quick_xml::events::Event;

let mut reader = Reader::from_reader(raw.as_slice());
reader.config_mut().trim_text(true);

let mut buf = Vec::new();
loop {
    match reader.read_event_into(&mut buf) {
        Ok(Event::Start(e)) => {
            let tag = e.local_name();  // compare tag.as_ref() with b"box"
            for attr in e.attributes().flatten() {
                let key = attr.key.as_ref();                    // &[u8]
                let val = String::from_utf8_lossy(&attr.value); // raw: entities NOT decoded
            }
        }
        Ok(Event::End(e)) => { /* closing tag */ }
        Ok(Event::Empty(e)) => { /* self-closing <tag/> */ }
        Ok(Event::Text(e)) => { let text = e.unescape()?; }
        Ok(Event::Eof) => break,
        Err(e) => { /* error at reader.error_position() */ }
        _ => {}
    }
    buf.clear();  // IMPORTANT: reuse buffer
}
```

### Key Points

- **No external entity support:** does NOT resolve DTD or external SYSTEM entities (safe against XXE)
- **Unescaping knows only the 5 predefined entities** (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`) and character references
- **Attribute values are not unescaped for you.** With the `encoding` feature (on in this crate) `unescape_value()` is compiled out; use `decode_and_unescape_value(reader.decoder())` or `quick_xml::escape::unescape` (as `gdml/parser.rs` does)
- **Buffer reuse** with `read_event_into` is critical for performance
- **Tag names are `&[u8]`:** compare with `b"tagname"`
- **`Event::Empty`** for self-closing tags (`<br/>`) by default

---

## 3. Serde / serde_json

### Key Derive Attributes

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    #[serde(rename = "type")]                    // Rust reserved word
    kind: String,
    #[serde(default)]                            // use Default if missing
    timeout_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip)]                               // omit entirely
    internal: bool,
    #[serde(flatten)]                            // inline nested struct
    metadata: Metadata,
}
```

### Container Attributes

| Attribute | Effect |
|-----------|--------|
| `rename_all = "camelCase"` | Naming convention for all fields |
| `deny_unknown_fields` | Reject unexpected keys |
| `tag = "type"` | Internally tagged enum |
| `untagged` | No tag, tries each variant |
| `default` | Default::default() for missing fields |

### serde_json APIs

```rust
let val: T = serde_json::from_str(json)?;     // from string
let val: T = serde_json::from_slice(bytes)?;   // from bytes (faster)
let json = serde_json::to_string(&val)?;
let json = serde_json::to_string_pretty(&val)?;

// Dynamic JSON
let v = json!({ "key": value, "list": [1, 2, 3] });
```

### Performance Tips

- **`from_slice` > `from_reader`:** can reference input directly
- Deserialize into typed structs, not `Value` intermediaries
- `untagged` enums are slow (tries each variant)
- `flatten` buffers entire object internally
- `&'a str` fields borrow from the input; `Cow<'a, str>` needs `#[serde(borrow)]` to do so

---

## 4. evalexpr

### Basic Usage

```rust
use evalexpr::*;

let result = eval("1 + 2 * 3")?;           // Value::Int(7)
let f = eval_float("3.14 * 2.0")?;         // 6.28

// Context with variables (v12+: numeric types are a type parameter)
let mut ctx = HashMapContext::<DefaultNumericTypes>::new();
ctx.set_value("x".into(), Value::from_int(10))?;
let result = eval_with_context("x * 2 + 1", &ctx)?;

// Pre-compiled (parse once, eval many)
let tree = build_operator_tree::<DefaultNumericTypes>("x * 2 + y")?;
tree.eval_with_context(&ctx)?;
```

### Custom Functions

```rust
ctx.set_function("double".into(), Function::new(|arg| {
    Ok(Value::Float(arg.as_number()? * 2.0))  // as_float() rejects an Int such as 21
}))?;
eval_with_context("double(21)", &ctx)?;
```

### Operators (by precedence)

| Prec | Operators |
|------|-----------|
| 120 | `^` (exponentiation, always returns float) |
| 110 | `-` (unary), `!` |
| 100 | `*`, `/`, `%` |
| 95 | `+`, `-` |
| 80 | `<`, `>`, `<=`, `>=`, `==`, `!=` |
| 75 | `&&` |
| 70 | `\|\|` |
| 50 | `=`, `+=`, `-=`, `*=`, `/=` |

### Built-in Functions

**Math:** `min`, `max`, `math::abs`, `floor`, `round`, `ceil`, `math::ln`, `math::log`, `math::log2`, `math::log10`, `math::exp`, `math::sqrt`, `math::cbrt`, `math::sin`, `math::cos`, `math::tan`, `math::asin`, `math::acos`, `math::atan`, `math::atan2`, `math::pow`, `math::hypot`

**String:** `len`, `str::to_lowercase`, `str::to_uppercase`, `str::trim`, `str::substring`

**Utility:** `if(cond, true_val, false_val)`, `typeof`, `contains`

### Gotchas

- **`2^2` returns `4.0` (float)**, not `4` (int)
- **Integer division truncates:** `1/2` is `0`, use `1.0/2` for float
- Multi-arg functions receive a single `Value::Tuple`
- `HashMapContext::set_value` refuses to change a variable's type (Int to Float)
- **License: AGPL-3.0-only**

---

## 5. Tokio

### spawn: Concurrent Tasks

```rust
let handle = tokio::spawn(async move {
    expensive_work().await
});
let result = handle.await?;
```

Future must be `Send + 'static`. Use `Arc` for shared ownership.

### RwLock vs Mutex

| | `tokio::sync::Mutex` | `tokio::sync::RwLock` | `std::sync::Mutex` |
|-|---------------------|----------------------|---------------------|
| Hold across `.await` | Yes | Yes | **No** |
| Multiple readers | No | Yes | No |
| Best for | IO resources | Read-heavy state | Quick non-async ops |

**Guidelines:**
- **Default to `std::sync::Mutex`** for fast operations not held across `.await`
- **`tokio::sync::RwLock`** when reads vastly outnumber writes
- **Never** hold `std::sync::MutexGuard` across `.await`

### spawn_blocking: CPU-Bound Work

```rust
let result = tokio::task::spawn_blocking(move || {
    cpu_intensive_work(data)
}).await?;
```

Use for: parsing large files, compression, cryptography, blocking I/O.
Cannot be aborted once running.
