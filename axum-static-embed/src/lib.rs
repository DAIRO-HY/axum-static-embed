use std::{
    env, fs,
    path::{Path, PathBuf},
};
use regex::NoExpand;

/// 生成静态资源路由代码块，并写入 OUT_DIR 以供 include! 使用。
const ROUTE_BLOCK_FILE_NAME: &str = "static_routes.rs";

/// 生成HTML代码，替换include部分，然后写入OUT_DIR/html目录
const HTML_OUT_DIR: &str = "html";

/// 参与编译期 gzip 压缩的文件扩展名（纯文本类资源，压缩收益明显）
const COMPRESSIBLE_EXTS: &[&str] = &["html", "htm", "css", "js", "mjs", "json", "svg", "xml", "txt", "map"];

/// 生成静态资源路由代码，如果是html，替换include部分，然后写入OUT_DIR/html目录
/// root_dir 是静态资源的根目录， max_age 是 Cache-Control 的 max-age 值（单位秒），
/// compress 是否在编译期对文本类资源（html/css/js等）做 gzip 压缩并附加 Content-Encoding 响应头
pub fn make(root_dir: &str, max_age: u32, compress: bool) {
    let root_dir = root_dir.replace("\\", "/"); // Windows 路径分隔符替换为 URL 斜杠

    // 写到 OUT_DIR
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join(HTML_OUT_DIR);
    let mut route = String::new();
    loop_files(Path::new(&root_dir), &mut |p| {
        let path_str = p.to_str().unwrap().to_string();
        if path_str.ends_with(".tpl.html") {
            // tpl文件不生成路由
            return;
        }

        let is_html = path_str.ends_with(".html");
        let should_compress = compress && is_compressible(&path_str);

        // 待写入 OUT_DIR 的内容；None 表示直接引用原始文件（未合并模板也未压缩）
        let mut out_bytes: Option<Vec<u8>> = None;

        if is_html {
            //读取并合并模板
            let (html, is_handled) = make_html(1, p);
            if is_handled || should_compress {
                out_bytes = Some(html.into_bytes());
            }
        } else if should_compress {
            out_bytes = Some(fs::read(p).unwrap());
        }

        if should_compress {
            out_bytes = out_bytes.map(|bytes| gzip_compress(&bytes));
        }

        let is_out_dir = out_bytes.is_some();
        if let Some(bytes) = &out_bytes {
            let out_path = out_dir.join(p);
            if let Some(parent) = out_path.parent() {
                if !parent.exists() {
                    // 确保父目录存在
                    fs::create_dir_all(parent).unwrap();
                }
            }
            println!(
                "cargo:warning=----------------------------->: {}",
                out_path.display()
            );
            fs::write(&out_path, bytes).unwrap();
        }

        //添加路由
        route.push_str(&make_route_item(&root_dir, max_age, p, is_out_dir, should_compress));
        // println!("{}", out_path.display());
    })
        .unwrap();

    // 写入路由函数代码
    write_html_route_src(route);
}

/// 让 Cargo 精确追踪 root_dir 下每个文件以及每一级子目录的变化，只有真正发生变化时
/// 才会重新执行 build script（而不是每次编译都重新执行）。建议在 build.rs 中与 [`make`] 搭配使用：
///
/// ```no_run
/// fn main() {
///     axum_static_embed::watch_dir("static");
///     axum_static_embed::make("static", 3600, true);
/// }
/// ```
///
/// 是否调用由开发者自行决定：不调用时，Cargo 会退化为默认策略——包内任意文件变化都会
/// 重新执行 build script，行为更保守但不会漏检。
pub fn watch_dir(root_dir: &str) {
    watch_dir_recursive(Path::new(root_dir)).unwrap();
}

/// 递归为 dir 本身以及下面的每个文件输出 cargo:rerun-if-changed 标记：
/// - 监听文件本身，内容被修改时触发重新构建；
/// - 监听目录本身，目录下新增/删除文件导致目录 mtime 变化，同样能触发重新构建。
fn watch_dir_recursive(dir: &Path) -> std::io::Result<()> {
    println!("cargo:rerun-if-changed={}", dir.display());
    for entry_res in fs::read_dir(dir)? {
        let entry = entry_res?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            watch_dir_recursive(&path)?;
        } else if metadata.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    Ok(())
}

/// 根据扩展名判断文件是否值得做 gzip 压缩（图片、字体等已压缩格式不在此列）
fn is_compressible(path_str: &str) -> bool {
    Path::new(path_str)
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| COMPRESSIBLE_EXTS.iter().any(|e| e.eq_ignore_ascii_case(ext)))
        .unwrap_or(false)
}

/// 使用最高压缩率对内容做 gzip 压缩（仅编译期执行一次，不影响运行时性能）
fn gzip_compress(data: &[u8]) -> Vec<u8> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

/// 合并模板：将 html 中的 {{template ...}} 替换为对应模板文件内容
/// deep 参数用于控制递归深度，防止无限循环（可选，根据实际情况调整或移除）, html_path 是当前处理的 HTML 文件路径，用于解析相对模板路径
/// return (处理后的 HTML 内容, 是否进行了替换)
fn make_html(deep: u8, html_path: &Path) -> (String, bool) {
    if deep > 10 {
        // 递归过深，可能是循环引用，报错并返回原内容
        panic!(
            "Error: Template recursion too deep at {}",
            html_path.display()
        );
    }
    //读取文件
    let mut html = fs::read_to_string(html_path).unwrap();
    let template_name_list = get_template_name(&html);
    if template_name_list.is_empty() {
        // 没有模板调用，直接返回
        return (html, false);
    }

    template_name_list.iter().for_each(|it| {
        html = replace_include(deep, &html, html_path, it);
    });
    (html, true)
}

/// 替换 html 中的 {{template include/head.template}} 部分
fn replace_include(deep: u8, html: &str, cur_path: &Path, include_path_str: &str) -> String {
    let include_path = cur_path.parent().unwrap().join(format!("{}.tpl.html" ,include_path_str));
    if !include_path.exists() {
        // 模板文件不存在，报错并跳过
        return html.to_string();
    }
    let (include_content, _) = make_html(deep + 1, &include_path); // 预先读取模板内容
    let mut regex_pattern = r"\{\{\s*template\s*[PATH]\s*\}\}".to_string();
    regex_pattern = regex_pattern
        .replace("[PATH]", include_path_str)
        .replace(".", "\\.");

    //替换 {{template include/head.template}} 这种格式的模板调用
    //被替换的内容不能包含 $ 符号，否则会被当作正则替换的变量，导致替换结果不正确，所以使用 NoExpand 包装替换内容
    let html_new = regex::Regex::new(&regex_pattern)
        .unwrap()
        .replace_all(&html, NoExpand(include_content.as_str()));
    html_new.to_string()
}

/// 从文本中提取 {{template ...}} 里的路径部分
/// 例如: "{{template include/head.template}}" -> "include/head.template"
pub fn get_template_name(input: &str) -> Vec<String> {
    // 解释：
    // \{\{       匹配开头的 {{
    // \s*        允许 {{ 后有空白
    // template   关键字 template
    // \s+        至少一个空白分隔
    // ([^}\s]+)  捕获路径：直到遇到空白或右花括号为止（常见路径不会包含空白或右花括号）
    // [^}]*      （可选）允许在路径后面有其它非 } 的字符（若模板语法允许附加参数）
    // \s*\}\}    结尾 }}
    let re = regex::Regex::new(r"\{\{\s*template\s+([^}\s]+)[^}]*\s*\}\}").unwrap();
    re.captures_iter(input)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

/// 遍历 dir 下的所有子孙“文件”（不含目录），对每个文件调用回调 f。
fn loop_files<F>(dir: &Path, f: &mut F) -> std::io::Result<()>
where
    F: FnMut(&Path),
{
    // read_dir 返回的是 Iterator<Item = io::Result<DirEntry>>
    for entry_res in fs::read_dir(dir)? {
        let entry = entry_res?; // 传播 read_dir 内部错误
        let path = entry.path();

        // 注意：symlink_metadata 不会跟随符号链接；metadata 会跟随
        let metadata = fs::symlink_metadata(&path)?;

        if metadata.is_dir() {
            // 递归进入子目录
            loop_files(&path, f)?;
        } else if metadata.is_file() {
            // 仅对文件回调
            f(&path);
        } else {
            // 其他类型（符号链接、设备文件等），按需处理或忽略
            // 如果想把指向文件的符号链接也当作文件，可改用 metadata() 并判断 is_file()
        }
    }
    Ok(())
}

/// 生成单个静态资源的路由代码块
/// root_dir 是静态资源的根目录， max_age 是 Cache-Control 的 max-age 值（单位秒）， file_path 是当前文件路径，
/// is_out_dir 表示是否已经写入 OUT_DIR， is_compressed 表示写入 OUT_DIR 的内容是否已 gzip 压缩（需附加 Content-Encoding 头）
fn make_route_item(
    root_dir: &str,
    max_age: u32,
    file_path: &Path,
    is_out_dir: bool,
    is_compressed: bool,
) -> String {
    const ROUTE_ITEM_TEMPLATE: &str = r##"
    .route("[ROUTE_PATH]", get(async || -> Response {
        (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "[MIME]"),
                (
                    header::CACHE_CONTROL,
                    "[CACHE_CONTROL]",
                ),
                [CONTENT_ENCODING]
            ],
            include_bytes!(concat!(env!("[PATH_ENV]"), "/[PATH]"))
        ).into_response()
    }))"##;

    let mut file_path_str = file_path.to_string_lossy().replace("\\", "/"); // Windows 路径分隔符替换为 URL 斜杠

    // include_bytes! 需要相对于 Cargo.toml 的路径，因此如果已经写入 OUT_DIR，则路径应该从 OUT_DIR 开始；否则从 CARGO_MANIFEST_DIR 开始
    let mut path_env = "CARGO_MANIFEST_DIR";
    if is_out_dir {
        file_path_str.insert_str(0, "/");
        file_path_str.insert_str(0, HTML_OUT_DIR);
        path_env = "OUT_DIR";
    }

    let mut route_path = file_path.to_string_lossy().replace("\\", "/"); // Windows 路径分隔符替换为 URL 斜杠
    route_path = route_path.strip_prefix(root_dir).unwrap().to_string(); // 去掉前缀路径部分

    //缓存头部标识
    let cache_control = if max_age > 0 {
        &format!("public, max-age={}, immutable", max_age)
    } else {
        "no-cache"
    };

    // 根据文件名猜测 content-type；找不到则 application/octet-stream
    let mime = mime_guess::from_path(&file_path_str).first_or_octet_stream();

    // 已在编译期 gzip 压缩的内容，需要告知客户端解码方式
    let content_encoding = if is_compressed {
        r#"(header::CONTENT_ENCODING, "gzip"),"#
    } else {
        ""
    };

    let item_src = &ROUTE_ITEM_TEMPLATE
        .replace("[ROUTE_PATH]", &route_path)
        .replace("[PATH]", &file_path_str)
        .replace("[PATH_ENV]", path_env)
        .replace("[MIME]", &mime.as_ref().to_string())
        .replace("[CACHE_CONTROL]", cache_control)
        .replace("[CONTENT_ENCODING]", content_encoding);
    item_src.to_string()
}

/// 生成静态资源路由代码块，并写入 OUT_DIR 以供 include! 使用
fn write_html_route_src(route_src: String) {
    //设置路由函数代码
    let set_routes_fnuc_template = r##"
/// Generated at [TIME]
/// 添加静态资源路由
fn add_static_routes(app: Router) -> Router {
    return app[ROUTER_ITEMS];
}"##;

    let set_routes_fnuc = set_routes_fnuc_template
        .replace("[ROUTER_ITEMS]", &route_src)
        .replace("[TIME]", &chrono::Local::now().to_rfc3339());

    // 写到 OUT_DIR
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_path = out_dir.join(ROUTE_BLOCK_FILE_NAME);

    println!(
        "cargo:warning=静态资源文件路由已写入文件: {}",
        out_path.display()
    );
    fs::write(&out_path, set_routes_fnuc).expect("write processed file failed");
}
