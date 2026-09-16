use tokio::process::Command;
use std::process::Stdio;
use std::error::Error;

pub async fn run_job(binary_path: &str, args: &[&str]) -> Result<bool, Box<dyn Error>> {
    log::info!("Spawning job: {} {:?}", binary_path, args);
    
    let mut child = Command::new(binary_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
        
    let status = child.wait().await?;
    
    if status.success() {
        log::info!("Job {} completed successfully.", binary_path);
        Ok(true)
    } else {
        log::error!("Job {} failed with status: {}", binary_path, status);
        Ok(false)
    }
}
