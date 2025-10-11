use anyhow::{Context, Result};
use clap::Parser;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tokio::time::sleep;

#[derive(Parser, Debug)]
#[command(name = "download-spect")]
#[command(about = "Downloads OpenAPI specs from apis.guru", long_about = None)]
struct Args {
    /// Number of simultaneous downloads
    #[arg(short = 'c', long, default_value_t = 3)]
    concurrency: usize,

    /// Sleep duration between downloads in seconds
    #[arg(short = 's', long, default_value_t = 0.5)]
    sleep_duration: f64,

    /// Output directory
    #[arg(short = 'o', long, default_value = "data")]
    output_dir: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
struct ApiSpec {
    versions: HashMap<String, VersionInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
struct VersionInfo {
    #[serde(rename = "swaggerUrl")]
    swagger_url: Option<String>,
    #[serde(rename = "swaggerYamlUrl")]
    swagger_yaml_url: Option<String>,
    info: InfoMetadata,
}

#[derive(Debug, Deserialize, Serialize)]
struct InfoMetadata {
    version: String,
}

type ApiList = HashMap<String, ApiSpec>;

#[derive(Debug, Clone)]
struct DownloadTask {
    provider: String,
    service: String,
    version: String,
    json_url: Option<String>,
    yaml_url: Option<String>,
}

async fn fetch_api_list() -> Result<ApiList> {
    let url = "https://api.apis.guru/v2/list.json";
    println!("Fetching API list from {}...", url);

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to fetch API list")?;

    let api_list: ApiList = response
        .json()
        .await
        .context("Failed to parse API list JSON")?;

    println!("Found {} APIs", api_list.len());
    Ok(api_list)
}

fn parse_api_key(key: &str) -> (String, String) {
    // API keys are in format "provider.com:serviceName"
    if let Some(pos) = key.find(':') {
        let provider = key[..pos].to_string();
        let service = key[pos + 1..].to_string();
        (provider, service)
    } else {
        // Fallback if no colon found
        ("unknown".to_string(), key.to_string())
    }
}

fn create_download_tasks(api_list: &ApiList) -> Vec<DownloadTask> {
    let mut tasks = Vec::new();

    for (api_key, api_spec) in api_list {
        let (provider, service) = parse_api_key(api_key);

        for (_version_key, version_info) in &api_spec.versions {
            tasks.push(DownloadTask {
                provider: provider.clone(),
                service: service.clone(),
                version: version_info.info.version.clone(),
                json_url: version_info.swagger_url.clone(),
                yaml_url: version_info.swagger_yaml_url.clone(),
            });
        }
    }

    tasks
}

async fn download_file(client: &reqwest::Client, url: &str, path: &PathBuf) -> Result<()> {
    // Check if file already exists
    if path.exists() {
        return Ok(());
    }

    let response = client
        .get(url)
        .send()
        .await
        .context(format!("Failed to download from {}", url))?;

    let content = response
        .bytes()
        .await
        .context("Failed to read response body")?;

    fs::write(path, content)
        .await
        .context(format!("Failed to write to {:?}", path))?;

    Ok(())
}

async fn process_download_task(
    client: &reqwest::Client,
    task: DownloadTask,
    output_dir: &PathBuf,
    sleep_duration: Duration,
) -> Result<()> {
    // Create directory structure: data/provider/service/version/
    let task_dir = output_dir
        .join(&task.provider)
        .join(&task.service)
        .join(&task.version);

    fs::create_dir_all(&task_dir)
        .await
        .context(format!("Failed to create directory {:?}", task_dir))?;

    let mut downloaded_count = 0;

    // Download JSON if available
    if let Some(json_url) = &task.json_url {
        let json_path = task_dir.join("spec.json");

        if json_path.exists() {
            println!(
                "Skipping JSON (exists): {}/{}/{}",
                task.provider, task.service, task.version
            );
        } else {
            println!(
                "Downloading JSON: {}/{}/{} from {}",
                task.provider, task.service, task.version, json_url
            );

            if let Err(e) = download_file(client, json_url, &json_path).await {
                eprintln!(
                    "Failed to download JSON for {}/{}/{}: {}",
                    task.provider, task.service, task.version, e
                );
            } else {
                downloaded_count += 1;
            }
        }
    }

    // Sleep between downloads only if we actually downloaded
    if downloaded_count > 0 {
        sleep(sleep_duration).await;
    }

    // Download YAML if available
    if let Some(yaml_url) = &task.yaml_url {
        let yaml_path = task_dir.join("spec.yaml");

        if yaml_path.exists() {
            println!(
                "Skipping YAML (exists): {}/{}/{}",
                task.provider, task.service, task.version
            );
        } else {
            println!(
                "Downloading YAML: {}/{}/{} from {}",
                task.provider, task.service, task.version, yaml_url
            );

            if let Err(e) = download_file(client, yaml_url, &yaml_path).await {
                eprintln!(
                    "Failed to download YAML for {}/{}/{}: {}",
                    task.provider, task.service, task.version, e
                );
            } else {
                downloaded_count += 1;
            }
        }
    }

    // Sleep after completing downloads only if we actually downloaded something
    if downloaded_count > 0 {
        sleep(sleep_duration).await;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("Configuration:");
    println!("  Concurrency: {}", args.concurrency);
    println!("  Sleep duration: {}s", args.sleep_duration);
    println!("  Output directory: {}", args.output_dir.display());
    println!();

    // Fetch the API list
    let api_list = fetch_api_list().await?;

    // Create download tasks
    let tasks = create_download_tasks(&api_list);
    println!("Total download tasks: {}", tasks.len());
    println!();

    // Create output directory
    fs::create_dir_all(&args.output_dir)
        .await
        .context("Failed to create output directory")?;

    // Create HTTP client
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("Failed to create HTTP client")?;

    let sleep_duration = Duration::from_secs_f64(args.sleep_duration);

    // Process downloads with concurrency limit
    stream::iter(tasks)
        .map(|task| {
            let client = client.clone();
            let output_dir = args.output_dir.clone();
            async move {
                process_download_task(&client, task, &output_dir, sleep_duration).await
            }
        })
        .buffer_unordered(args.concurrency)
        .collect::<Vec<_>>()
        .await;

    println!("\nDownload complete!");
    Ok(())
}
