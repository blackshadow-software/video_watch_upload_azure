use main::run;

pub mod main;

pub async fn watch_and_upload(
    watch_path: &str,
    account: &str,
    container: &str,
    token: &str,
) -> Result<(), String> {
    run(watch_path, account, container, token).await
}
