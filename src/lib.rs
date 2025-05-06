use main::run;

pub mod main;

pub async fn watch_and_upload() -> Result<(), String> {
    run("", "", "", "").await
}
