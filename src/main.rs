use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Html,
    routing::{get},
    Router,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event};
use pulldown_cmark::{html, Parser};
use std::{
    sync::{Arc, Mutex},
    path::PathBuf,
};
use tokio::sync::broadcast;

#[tokio::main]
async fn main() {
    let (tx, _) = broadcast::channel(10);
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        watch_markdown_file(tx_clone, "index.md".into()).await;
    });
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/ws", get(move |ws: WebSocketUpgrade| handle_websocket(ws, tx.clone())));
    println!("Sunucu başlatıldı: http://localhost:3000");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn serve_index() -> Html<String> {
    Html(
        "<!DOCTYPE html>
<html>
<head>
  <title>Rust Live MD</title>
  <script>
    let ws = new WebSocket('ws://localhost:3000/ws');
    ws.onmessage = (event) => {
      document.getElementById('content').innerHTML = event.data;
    };
  </script>
</head>
<body>
  <div id='content'>Yükleniyor...</div>
</body>
</html>".to_string(),
    )
}

async fn handle_websocket(ws: WebSocketUpgrade, tx: broadcast::Sender<String>) -> impl axum::response::IntoResponse {
    let rx = tx.subscribe(); // Yeni bir Receiver oluştur
    ws.on_upgrade(move |socket| websocket_handler(socket, rx))
}

async fn websocket_handler(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    while let Ok(markdown_html) = rx.recv().await {
        if socket.send(Message::Text(markdown_html)).await.is_err() {
            break;
        }
    }
}

async fn watch_markdown_file(tx: broadcast::Sender<String>, file_path: PathBuf) {
    let file_path = file_path.canonicalize().unwrap();
    let (watcher_tx, mut watcher_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if watcher_tx.send(res).is_err() {
                eprintln!("Watcher kanalına mesaj gönderilemedi.");
            }
        },
        notify::Config::default(),
    ).expect("Watcher oluşturulamadı!");
    watcher
        .watch(&file_path, RecursiveMode::NonRecursive)
        .expect("Dosya izlenemedi!");
    while let Some(Ok(event)) = watcher_rx.recv().await {
        if let Some(path) = event.paths.first() {
            if path.ends_with(&file_path) {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    let html_output = markdown_to_html(&content);
                    let _ = tx.send(html_output);
                }
            }
        }
    }
}

fn markdown_to_html(markdown: &str) -> String {
    let parser = Parser::new(markdown);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}
