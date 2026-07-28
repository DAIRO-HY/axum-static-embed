# axum-static-embed

[![Crates.io](https://img.shields.io/crates/v/axum-static-embed.svg)](https://crates.io/crates/axum-static-embed)
[![Docs.rs](https://docs.rs/axum-static-embed/badge.svg)](https://docs.rs/axum-static-embed)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

在**编译期**把静态资源（HTML/CSS/JS/图片等）打包进二进制文件，并自动生成对应的
[Axum](https://github.com/tokio-rs/axum) 路由代码 —— 部署时只需要一个可执行文件，
不依赖运行时的静态资源目录。

作为 `build-dependencies` 在 `build.rs` 中调用，扫描指定目录后生成
`add_static_routes(app: Router) -> Router` 的 Rust 源码，业务代码通过 `include!` 引入即可。

## 这个仓库包含什么

这是一个 Cargo workspace，包含两个 crate：

| 目录 | 说明 |
| --- | --- |
| [`axum-static-embed`](axum-static-embed) | 核心库，发布在 [crates.io](https://crates.io/crates/axum-static-embed) |
| [`axum-static-embed-sample`](axum-static-embed-sample) | 可直接 `cargo run` 的完整示例项目 |

## 特性

- **零运行时依赖**：所有资源通过 `include_bytes!` 编译进二进制。
- **HTML 模板合并**：`{{template path}}` 语法在编译期合并公共片段（导航栏、页头页脚等）。
- **编译期 gzip 压缩**：文本类资源（html/css/js/json/svg 等）自动压缩并附加
  `Content-Encoding: gzip` 响应头。
- **自动 Content-Type**：基于文件扩展名推断。
- **可配置缓存策略**：统一设置 `Cache-Control` 的 `max-age`（或 `no-cache`）。

## 快速开始

```toml
[build-dependencies]
axum-static-embed = "0.0.1"

[dependencies]
axum = "0.8"
```

```rust
// build.rs
fn main() {
    axum_static_embed::watch_dir("static");
    axum_static_embed::make("static", 3600, true);
}
```

```rust
// src/main.rs
use axum::{
    Router,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

include!(concat!(env!("OUT_DIR"), "/static_routes.rs"));

#[tokio::main]
async fn main() {
    let app = add_static_routes(Router::new());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

参数说明、模板语法、工作原理等完整文档见 [`axum-static-embed/README.md`](axum-static-embed/README.md)。
想直接跑起来看效果，运行：

```bash
cargo run -p axum-static-embed-sample
```

然后访问 http://127.0.0.1:3000/index.html，详见
[`axum-static-embed-sample/README.md`](axum-static-embed-sample/README.md)。

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
