use std::{fs, path::PathBuf, thread, time::Duration};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use reqwest::Client;
use tokio::{runtime::Runtime, sync::mpsc};

#[tokio::main]
async fn main() {
    match run("", "", "", "").await {
        Ok(_) => println!("Success!"),
        Err(e) => eprintln!("Error: {}", e),
    }
}

pub async fn run(
    watch_path: &str,
    account: &str,
    container: &str,
    token: &str,
) -> Result<(), String> {
    let path = PathBuf::from(watch_path);

    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }

    let (tx, mut rx) = mpsc::channel(32);

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            futures::executor::block_on(tx.send(res)).unwrap();
        },
        notify::Config::default(),
    )
    .map_err(|e| format!("Failed to initialize watcher: {}", e))?;

    watcher
        .watch(&path, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch path: {}", e))?;

    println!("Watching directory: {}", path.display());

    while let Some(event_result) = rx.recv().await {
        match event_result {
            Ok(event) => match event.clone().kind {
                EventKind::Create(_) | EventKind::Modify(_) => {
                    for path in event.paths.iter() {
                        let path_str = path.clone().display().to_string();
                        let path = PathBuf::from(path);
                        if path.extension().map_or(false, |ext| ext == "mp4") {
                            if is_file_complete(&path) {
                                println!("✅ File complete: {}", &path.display());
                                let file_name = path_str
                                    .split('/')
                                    .last()
                                    .unwrap()
                                    .split('\\')
                                    .last()
                                    .unwrap();
                                let url = format!(
                                    "https://{}.blob.core.windows.net/{}/{}{}",
                                    account, container, file_name, token
                                );
                                println!("Uploading to: {}", url);
                                thread::spawn(move || {
                                    Runtime::new().unwrap().block_on(async move {
                                        // match upload_video_to_azure(&url, &path_str).await {
                                        //     Ok(_) => {
                                        //         println!("File uploaded successfully: {}", path_str)
                                        //     }
                                        //     Err(e) => eprintln!("Failed to upload file: {}", e),
                                        // }
                                    })
                                });
                            } else {
                                println!("⚠️ File not complete yet: {}", path.display());
                            }
                        }
                    }
                }
                _ => (),
            },
            Err(e) => eprintln!("Watcher error: {}", e),
        }
    }
    Ok(())
}

fn is_file_complete(path: &PathBuf) -> bool {
    let mut previous_size = 0;

    for _ in 0..5 {
        thread::sleep(Duration::from_secs(1));

        match fs::metadata(&path) {
            Ok(metadata) => {
                let current_size = metadata.len();
                if current_size == previous_size && current_size > 0 {
                    return true;
                }
                previous_size = current_size;
            }
            Err(_) => return false,
        }
    }

    false
}

async fn upload_video_to_azure(url: &str, path: &str) -> Result<(), String> {
    match tokio::fs::read(path).await {
        Ok(file_bytes) => {
            let client = Client::new();
            let response = client
                .put(url)
                .header("x-ms-blob-type", "BlockBlob")
                .header("Content-Type", "video/mp4")
                .body(file_bytes)
                .send()
                .await;
            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        println!("File uploaded successfully: {}", path);
                        Ok(())
                    } else {
                        Err(format!("Failed to upload file: {}", resp.status()))
                    }
                }
                Err(e) => Err(format!("Failed to upload file: {}", e)),
            }
        }
        Err(e) => return Err(format!("Failed to read file: {}", e)),
    }
}
