use std::{fs::create_dir_all, path::PathBuf, process::{ExitStatus, Stdio}, sync::Arc};

use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter}, sync::{Mutex, broadcast},
};
use tokio_stream::StreamExt;

const BRIGADIER_URL: &str =
    "https://github.com/timsutton/brigadier/releases/download/0.2.6/brigadier.exe";
#[cfg(all(target_os = "windows", target_arch = "aarch64"))]
const SEVENZIP_INSTALLER_URL: &str = "https://www.7-zip.org/a/7z2600-arm64.exe";
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const SEVENZIP_INSTALLER_URL: &str = "https://www.7-zip.org/a/7z2600-x64.exe";
#[cfg(all(target_os = "windows", target_arch = "x86"))]
const SEVENZIP_INSTALLER_URL: &str = "https://www.7-zip.org/a/7z2600.exe";

const SEVENZIP_RELATIVE_PATH: [&str; 3] = ["Program Files", "7-Zip", "7z.exe"];

struct AppData {
    data_path: PathBuf,
    brigadier_data_path: PathBuf,
    brigadier_path: PathBuf,
    kill_brigadier_sender: broadcast::Sender<()>,
}

#[tauri::command]
fn get_os() -> String {
    #[cfg(target_os = "windows")]
    {
        "windows".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        "macos".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "linux".to_string()
    }
}

#[tauri::command]
#[cfg(target_os = "windows")]
async fn install_sevenzip(app: AppHandle) -> Result<(), String> {
    let app_data = app.state::<AppData>();

    let sevenzip_installer_path = app_data.data_path.join("7zip-installer.exe");

    download_file(SEVENZIP_INSTALLER_URL, &sevenzip_installer_path)
        .await
        .map_err(|e| format!("Failed to download 7-Zip installer: {}", e))?;

    let mut cmd = std::process::Command::new(sevenzip_installer_path);

    cmd.arg("/S");

    elevated_process::Process::spawn(cmd)
        .wait()
        .await
        .map_err(|e| format!("Failed to run 7-Zip installer: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn pull_drivers(app: AppHandle, install: bool, model: Option<String>) -> Result<(), String> {
    let app_data = app.state::<AppData>();

    check_brigadier(&app_data.brigadier_path).await?;

    let brigadier_log_dir_path = app_data.data_path.join("logs");
    let brigadier_log_path = brigadier_log_dir_path.join("brigadier.txt");

    if brigadier_log_dir_path.is_file() {
        eprintln!(
            "Found file at: {}",
            brigadier_log_dir_path.to_string_lossy().to_string()
        );

        return Err(format!(
            "Found file at: {}",
            brigadier_log_dir_path.to_string_lossy().to_string()
        ));
    }

    if !brigadier_log_dir_path.is_dir() {
        tokio::fs::create_dir_all(&brigadier_log_dir_path)
            .await
            .map_err(|e| format!("Error creating log directory: {}", e))?;
    }

    if brigadier_log_path.is_dir() {
        eprintln!(
            "Found directory at: {}",
            brigadier_log_dir_path.to_string_lossy().to_string()
        );

        return Err(format!(
            "Found directory at: {}",
            brigadier_log_dir_path.to_string_lossy().to_string()
        ));
    }

    let l = File::create(brigadier_log_path)
        .await
        .map_err(|e| format!("Error creating brigadier output log file: {}", e))?;

    let mut cmd = tokio::process::Command::new(&app_data.brigadier_path.to_string_lossy().to_string());

    if install {
        cmd.arg("--install");
    }

    if let Some(m) = model {
        cmd.args(["--model", &m]);
    }

    cmd.current_dir(&app_data.brigadier_data_path);

    cmd.stderr(Stdio::from(l.into_std().await));

    let mut kill_rx = app_data.kill_brigadier_sender.subscribe();

    let proc = Arc::new(Mutex::new(cmd
        .spawn()
        .map_err(|e| format!("Error while running brigadier: {}", e))?
    ));

    let mut proc_guard = proc.lock().await;

    let status: ExitStatus = tokio::select! {
        e = proc_guard.wait() => {
            e.map_err(|e| format!("Error while running brigadier: {}", e))?
        }
        _ = kill_rx.recv() => {
            eprintln!("brigadier termination request received");
            
            drop(proc_guard);

            let id = proc.lock().await.id().ok_or("Failed to get process ID")?;

            kill_tree::tokio::kill_tree(id).await.map_err(|e| format!("Failed to kill process: {}", e))?;

            eprintln!("Target eliminated.");

            return Err("Job Cancelled".to_string());
        }
    };

    if !status.success() {
        let r = app.opener().open_path(
            &brigadier_log_dir_path.to_string_lossy().to_string(),
            None::<&str>,
        );

        if let Err(e) = r {
            eprintln!("Error opening directory: {}", e.to_string());
        }

        return Err("An error occurred while running brigadier".to_string());
    }

    let r = app.opener().open_path(
        &app_data.brigadier_data_path.to_string_lossy().to_string(),
        None::<&str>,
    );

    if let Err(e) = r {
        eprintln!("Error opening directory: {}", e.to_string());
    }

    Ok(())
}

#[tauri::command]
async fn kill_brigadier(app: AppHandle) {
    let app_data = app.state::<AppData>();

    let kill_tx = app_data.kill_brigadier_sender.clone();

    let _ = kill_tx.send(());
}

async fn check_brigadier(brigadier_path: &std::path::Path) -> Result<(), String> {
    if brigadier_path.is_file() {
        eprintln!("Brigadier already exists at: {:?}", brigadier_path);
    } else {
        eprintln!("Brigadier not found at: {:?}", brigadier_path);

        download_file(BRIGADIER_URL, brigadier_path)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

async fn download_file(
    url: &str,
    destination: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = reqwest::get(url).await?;

    if !response.status().is_success() {
        return Err(format!("Failed to download file: HTTP {}", response.status()).into());
    }

    let file = File::create(destination)
        .await
        .map_err(|e| format!("Failed to download file: {}", e))?;

    let mut writer = BufWriter::new(file);

    let mut stream = response.bytes_stream();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("Error while downloading: {}", e))?;
        writer
            .write(&chunk)
            .await
            .map_err(|e| format!("Error while writing file: {}", e))?;
    }

    writer
        .flush()
        .await
        .map_err(|e| format!("Error while flushing: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn sevenzip_installed() -> bool {
    let systemdrive = match std::env::var("SYSTEMDRIVE") {
        Ok(o) => o,
        Err(_) => {
            eprintln!("Failed to get system drive (via \"SYSTEMDRIVE\" environment variable)");

            return false;
        }
    };

    let mut path = PathBuf::from(systemdrive);

    path.push("\\");

    for c in SEVENZIP_RELATIVE_PATH {
        path.push(c);
    }

    if path.is_dir() {
        eprintln!(
            "Found a directory where the 7-Zip executable should be: {}",
            path.to_string_lossy().to_string()
        );

        return false;
    }

    if path.is_file() {
        eprintln!(
            "Found 7-Zip executable at: {}",
            path.to_string_lossy().to_string()
        );

        true
    } else {
        false
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_path = app.path().app_local_data_dir().unwrap();

            eprintln!("Data path: {}", data_path.to_string_lossy().to_string());

            if !data_path.is_dir() {
                create_dir_all(&data_path).unwrap();
            }

            let brigadier_data_path = data_path.join("brigadier_data");
            let brigadier_path = data_path.join("brigadier.exe");

            if !brigadier_data_path.is_dir() {
                if let Err(e) = create_dir_all(&brigadier_data_path) {
                    eprintln!("Error creating data directory: {}", e);
                };
            }

            let kill_brigadier_sender = broadcast::channel::<()>(1).0;

            app.manage(AppData {
                data_path,
                brigadier_data_path,
                brigadier_path,
                kill_brigadier_sender,
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_os,
            pull_drivers,
            kill_brigadier,
            sevenzip_installed,
            #[cfg(target_os = "windows")]
            install_sevenzip,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
