use axum::{
    Router,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

// build.rs 调用 axum_static_embed::make("static", 3600, true) 生成的路由代码，
// 定义了 fn add_static_routes(app: Router) -> Router
include!(concat!(env!("OUT_DIR"), "/static_routes.rs"));

#[tokio::main]
async fn main() {
    let app = Router::new();
    let app = add_static_routes(app);

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("listening on http://{addr}, try http://127.0.0.1:3000/index.html");
    axum::serve(listener, app).await.unwrap();
}
