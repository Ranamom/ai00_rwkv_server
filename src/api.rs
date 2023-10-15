use crate::{request_info, ReloadRequest, RuntimeInfo, ThreadRequest, ThreadState};
use axum::{extract::State, Json};
use memmap::Mmap;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Cursor,
    path::{Path, PathBuf},
};
#[derive(Debug, Serialize)]
pub struct ModelChoice {
    pub object: String,
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct ModelResponse {
    pub data: Vec<ModelChoice>,
}

pub async fn models(State(ThreadState(sender)): State<ThreadState>) -> Json<ModelResponse> {
    let model_name = request_info(sender)
        .map(|info| info.reload.path)
        .and_then(|path| path.file_name().map(|name| name.to_os_string()))
        .and_then(|name| name.into_string().ok())
        .unwrap_or_default();

    Json(ModelResponse {
        data: vec![ModelChoice {
            object: "models".into(),
            id: model_name,
        }],
    })
}

#[derive(Debug, Default, Serialize)]
#[serde(untagged)]
pub enum InfoResponse {
    Some(RuntimeInfo),
    #[default]
    None,
}

pub async fn info(State(ThreadState(sender)): State<ThreadState>) -> Json<InfoResponse> {
    match request_info(sender) {
        Some(info) => Json(InfoResponse::Some(info)),
        None => Json(InfoResponse::None),
    }
}

pub async fn load(
    State(ThreadState(sender)): State<ThreadState>,
    Json(request): Json<ReloadRequest>,
) -> Json<InfoResponse> {
    let _ = sender.send(ThreadRequest::Reload(request));
    info(State(ThreadState(sender))).await
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileInfoRequest {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(untagged)]
pub enum FileInfoResponse {
    Accepted(Vec<FileInfo>),
    #[default]
    Denied,
}

pub async fn files(
    State(ThreadState(_)): State<ThreadState>,
    Json(request): Json<FileInfoRequest>,
) -> Json<FileInfoResponse> {
    if request.path.is_dir() && request.path.starts_with("assets/") {
        let files = match std::fs::read_dir(request.path) {
            Ok(dir) => dir
                .filter_map(|x| x.ok())
                .filter(|x| x.path().is_file())
                .filter_map(|x| Some((x.file_name(), x.metadata().ok()?)))
                .map(|(name, meta)| FileInfo {
                    name: name.to_string_lossy().into_owned(),
                    size: meta.len(),
                })
                .collect(),
            Err(_) => Vec::new(),
        };
        Json(FileInfoResponse::Accepted(files))
    } else {
        Json(FileInfoResponse::Denied)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnzipRequest {
    pub target_dir: PathBuf,
    pub zip_path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct UnzipResponse {
    pub state: String,
}

pub async fn unzip(
    State(ThreadState(_)): State<ThreadState>,
    Json(request): Json<UnzipRequest>,
) -> Json<UnzipResponse> {
    if Path::new(&request.target_dir).exists() {
        // If exists, remove it
        fs::remove_dir_all(&request.target_dir).unwrap();
    }
    fs::create_dir_all(&request.target_dir).unwrap();

    let file = File::open(&request.zip_path).unwrap();
    let map = unsafe { Mmap::map(&file).unwrap() };
    match zip_extract::extract(Cursor::new(&map), &request.target_dir, false) {
        Ok(_) => Json(UnzipResponse { state: "OK".into() }),
        Err(_) => Json(UnzipResponse {
            state: "ERR".into(),
        }),
    }
}
