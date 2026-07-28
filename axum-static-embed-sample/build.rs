fn main() {
    // 追踪 static 目录下的文件/子目录变化，只有真正发生变化时才会重新执行下面的 make()
    axum_static_embed::watch_dir("static");

    // root_dir = "static"，max_age = 3600 秒，compress = true（对 html/css/js 等文本资源做编译期 gzip）
    axum_static_embed::make("static", 3600, true);
}
