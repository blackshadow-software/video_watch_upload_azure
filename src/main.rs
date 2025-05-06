use crc32fast::Hasher;
use indexmap::IndexMap;
use reqwest::Client;
use std::{fs, path::PathBuf, thread, time::Duration};
use tokio::{runtime::Runtime, time};

const DURATION: u64 = 360; // seconds
const WATCH_PATH: &str = "c:/Users/nextr/Downloads";
const ACCOUNT: &str = "";
const CONTAINER: &str = "video";
const TOKEN: &str = "";

#[tokio::main]
async fn main() {
    match run(WATCH_PATH, ACCOUNT, CONTAINER, TOKEN).await {
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
    let mut file_index: IndexMap<String, u32> = IndexMap::new();

    loop {
        println!("Checking directory: {}", path.display());

        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.extension().map_or(false, |ext| ext == "mp4") {
                    if let Some(current_crc) = compute_crc32(&path) {
                        let path_str = path.to_string_lossy().to_string();

                        match file_index.get(&path_str) {
                            Some(&saved_crc) if saved_crc == current_crc => {
                                println!("File unchanged: {}", path_str);
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
                                upload_video_to_azure(&url, &path_str);
                                file_index.shift_remove_entry(&path_str);
                            }
                            Some(_) => {
                                println!("File modified: {}", path_str);
                                file_index.insert(path_str, current_crc);
                            }
                            None => {
                                println!("New file detected: {}", path_str);
                                file_index.insert(path_str, current_crc);
                            }
                        }
                    }
                }
            }
        } else {
            eprintln!("Failed to read directory: {}", path.display());
        }
        println!("File indexs len: {:?}", file_index.len());
        time::sleep(Duration::from_secs(DURATION)).await;
    }
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

fn upload_video_to_azure(url: &str, path: &str) {
    let url = url.to_string();
    let path = path.to_string();
    let hanler = thread::spawn(move || {
        Runtime::new().unwrap().block_on(async move {
            match tokio::fs::read(&path).await {
                Ok(file_bytes) => {
                    let client = Client::new();
                    let response = client
                        .put(&url)
                        .header("x-ms-blob-type", "BlockBlob")
                        .header("Content-Type", "video/mp4")
                        .body(file_bytes)
                        .send()
                        .await;
                    match response {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                println!("File {} is uploaded successfully to : {}", path, url);
                                match tokio::fs::remove_file(&path).await {
                                    Ok(_) => println!("File deleted successfully: {}", path),
                                    Err(e) => eprintln!("Failed to delete file: {}", e),
                                }
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
        })
    });
    match hanler.join() {
        Ok(result) => match result {
            Ok(_) => println!("Upload thread completed successfully."),
            Err(e) => println!("Upload thread error: {}", e),
        },
        Err(e) => println!("Failed to join upload thread: {:?}", e),
    }
}

fn compute_crc32(path: &PathBuf) -> Option<u32> {
    let file_data = fs::read(path).ok()?;
    let mut hasher = Hasher::new();
    hasher.update(&file_data);
    Some(hasher.finalize())
}
