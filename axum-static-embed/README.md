# axum-static-embed

[![Crates.io](https://img.shields.io/crates/v/axum-static-embed.svg)](https://crates.io/crates/axum-static-embed)
[![Docs.rs](https://docs.rs/axum-static-embed/badge.svg)](https://docs.rs/axum-static-embed)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

在**编译期**把静态资源（HTML/CSS/JS/图片等）打包进二进制文件，并自动生成对应的
[Axum](https://github.com/tokio-rs/axum) 路由代码 —— 无需运行时依赖文件系统，
部署时只有一个可执行文件。

作为 `build-dependencies` 在 `build.rs` 中调用，扫描指定目录后生成一段
`add_static_routes(app: Router) -> Router` 的 Rust 源码，写入 `OUT_DIR`，
业务代码通过 `include!` 引入即可。

## 特性

- **零运行时依赖**：所有资源通过 `include_bytes!` 编译进二进制，容器镜像可以只有一个可执行文件。
- **HTML 模板合并**：`{{template path}}` 语法在编译期把公共片段（如导航栏、页头页脚）
  合并进页面，避免重复维护。
- **编译期 gzip 压缩**：对文本类资源（html/css/js/json/svg 等）自动压缩，减小
  二进制体积，同时自动附加 `Content-Encoding: gzip` 响应头。
- **自动 Content-Type**：基于文件扩展名使用 [`mime_guess`](https://crates.io/crates/mime_guess) 推断。
- **可配置缓存策略**：统一设置 `Cache-Control` 的 `max-age`（或 `no-cache`）。

## 安装

```toml
[build-dependencies]
axum-static-embed = "0.0.1"

[dependencies]
axum = "0.8"
```

`axum-static-embed` 本身不依赖 axum —— 它只在编译期生成源码文本，真正的
`axum::Router` / `axum::routing::get` 等类型是在**你的 crate**里被引用的，
所以 axum 需要作为普通依赖加到你的项目里。

## 用法

**目录结构**

```
your_project/
├── build.rs
├── src/
│   └── main.rs
└── static/
    ├── index.html
    ├── include/
    │   └── nav.tpl.html
    ├── css/style.css
    └── js/app.js
```

**`build.rs`**

```rust
fn main() {
    // 追踪 static 目录下文件/子目录的变化，只有真正发生变化时才会重新执行下面的 make()
    axum_static_embed::watch_dir("static");

    // root_dir, max_age(秒), compress
    axum_static_embed::make("static", 3600, true);
}
```

**`src/main.rs`**

```rust
use axum::{
    Router,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

// 引入生成的 fn add_static_routes(app: Router) -> Router
include!(concat!(env!("OUT_DIR"), "/static_routes.rs"));

#[tokio::main]
async fn main() {
    let app = Router::new();
    let app = add_static_routes(app);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

`static/index.html` 里的路径会被映射成去掉 `root_dir` 前缀的路由，例如
`static/css/style.css` -> `GET /css/style.css`。

完整可运行示例见 [`axum-static-embed-sample`](../axum-static-embed-sample)。

## `make(root_dir, max_age, compress)`

| 参数 | 说明 |
| --- | --- |
| `root_dir` | 静态资源根目录（相对于 `Cargo.toml` 所在目录） |
| `max_age` | `Cache-Control` 的 `max-age`（秒）；传 `0` 则响应头为 `no-cache` |
| `compress` | 是否对文本类资源在编译期做 gzip 压缩 |

参与压缩的扩展名：`html htm css js mjs json svg xml txt map`。其余类型
（图片、字体等已压缩格式）始终原样通过 `include_bytes!` 引用源文件，不做拷贝。

## `watch_dir(root_dir)`

对 `root_dir` 下的每个文件、每一级子目录输出 `cargo:rerun-if-changed`，让 Cargo
精确追踪变化，只有资源真正发生增/删/改时才会重新执行 `make()`，避免每次编译都
重新生成路由代码。是否调用完全由开发者决定——不调用时，Cargo 会退化为默认策略
（包内任意文件变化都会重新执行 build script），行为更保守但不会漏检。

## 模板合并语法

在 `.html` 文件中使用：

```html
{{template include/nav}}
```

会被替换为同目录下 `include/nav.tpl.html` 的内容（支持嵌套，最大递归深度 10）。
以 `.tpl.html` 结尾的文件本身**不会**生成路由，只作为被合并的片段存在。

> ⚠️ 替换是对整个文件内容做正则全文匹配替换，同一个 `{{template ...}}` 调用
> 在文件里出现几次就会被替换几次（包括出现在注释、代码示例文本中的情况），
> 使用时请留意。

## 工作原理

1. 递归扫描 `root_dir` 下所有文件。
2. 对 `.html` 文件做模板合并；对满足压缩条件的文本资源做 gzip 压缩。
3. 被处理过的内容写入 `OUT_DIR`，未被处理的文件保持原样，直接引用
   `CARGO_MANIFEST_DIR` 下的源文件，避免重复拷贝。
4. 为每个文件生成一段 Axum 路由，写入 `OUT_DIR/static_routes.rs`，
   合并为 `add_static_routes(app: Router) -> Router` 函数供 `include!` 使用。

## MSRV

需要支持 async closures（`edition = "2024"`），建议使用较新版本的稳定 Rust
工具链（1.85 及以上）。

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
