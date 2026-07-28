# axum-static-embed-sample

[`axum-static-embed`](../axum-static-embed) 的最小可运行示例，演示如何在编译期把静态资源
（HTML/CSS/JS/图片）打包进二进制文件，并自动生成 Axum 路由。

## 目录结构

```
static/               静态资源根目录（root_dir）
├── index.html        使用 {{template ...}} 引入模板片段
├── include/
│   └── nav.tpl.html   模板片段，以 .tpl.html 结尾的文件不会生成路由，只用于被合并
├── css/style.css
├── js/app.js
└── favicon.png
build.rs               构建期调用 axum_static_embed::watch_dir("static") + make("static", 3600, true)
src/main.rs             include! 生成的路由代码并启动 Axum 服务
```

## 运行

```bash
cargo run -p axum-static-embed-sample
```

然后访问 http://127.0.0.1:3000/index.html

## 演示的能力

- **模板合并**：`index.html` 里的 `{{template include/nav}}` 会在编译期被替换为
  `include/nav.tpl.html` 的内容（注意：替换是对全文做正则匹配，同一个模板调用出现几次就会被替换几次）。
- **编译期 gzip 压缩**：`index.html`、`style.css`、`app.js` 是文本类资源，会被自动
  gzip 压缩并附加 `Content-Encoding: gzip` 响应头。
- **原样透传**：`favicon.png` 不在压缩白名单内，会直接通过 `include_bytes!` 引用
  源文件，不产生额外拷贝。
- **缓存头**：所有资源都带有 `Cache-Control: public, max-age=3600, immutable`
  （由传给 `make()` 的 `max_age` 参数决定；传 `0` 则为 `no-cache`）。
