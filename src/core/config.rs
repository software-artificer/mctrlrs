use actix_web::http::uri;
use std::{env, fs, io, net, os::unix::fs::FileTypeExt, path};

#[derive(serde::Deserialize)]
struct ConfigFile {
    listen_on: net::SocketAddr,
    worlds_path: path::PathBuf,
    current_world_path: path::PathBuf,
    server_socket_path: path::PathBuf,
    users_file_path: path::PathBuf,
    #[serde(with = "http_serde::uri")]
    base_url: uri::Uri,
    #[serde(default = "default_min_password_len")]
    min_password_length: u8,
    #[serde(default = "default_max_password_len")]
    max_password_length: u8,
}

fn default_min_password_len() -> u8 {
    10
}

fn default_max_password_len() -> u8 {
    128
}

#[derive(thiserror::Error, Debug)]
pub enum LoadConfigError {
    #[error("Failed to obtain current working directory")]
    CurrentWorkingDir(#[source] io::Error),
    #[error("Failed to obtain absolute path for the binary")]
    ExecutablePath(#[source] io::Error),
    #[error("Failed to parse configuration file")]
    ParseFailure(#[from] serde_yml::Error),
    #[error("Failed to read configuration file contents {}", .path.display())]
    ReadError {
        path: path::PathBuf,
        source: io::Error,
    },
    #[error("Failed to canonicalize the path {path}: {}", .source)]
    CanonicalizePath {
        path: path::PathBuf,
        source: io::Error,
    },
    #[error("Failed to validate configuration file: {}", .0)]
    Validate(#[from] ConfigValidationError),
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigValidationError {
    #[error("Invalid worlds path: {}", .0)]
    WorldsPath(String),
    #[error("Invalid current world name: {}", .0)]
    CurrentWorldName(String),
    #[error("Invalid server socket path: {}", .0)]
    ServerSocketPath(String),
    #[error("Invalid users file path: {}", .0)]
    UsersFilePath(String),
    #[error("Invalid base URL: {}", .0)]
    InvalidBaseUrl(uri::Uri),
}

pub struct AppConfig {
    pub worlds_path: path::PathBuf,
    pub current_world_path: path::PathBuf,
    pub server_socket_path: path::PathBuf,
    pub users_file_path: path::PathBuf,
    pub base_url: uri::Uri,
    pub min_password_length: usize,
    pub max_password_length: usize,
}

pub struct Config {
    pub listen_on: net::SocketAddr,
    pub app_config: AppConfig,
}

impl Config {
    pub fn load<P: AsRef<path::Path>>(path: P) -> Result<Self, LoadConfigError> {
        let path = relative_path_to_absolute(path)?;
        let config_reader =
            fs::File::open(&path).map_err(|source| LoadConfigError::ReadError { path, source })?;
        let config: ConfigFile =
            serde_yml::from_reader(config_reader).map_err(LoadConfigError::ParseFailure)?;

        config.try_into().map_err(LoadConfigError::Validate)
    }
}

impl TryFrom<ConfigFile> for Config {
    type Error = ConfigValidationError;

    fn try_from(config: ConfigFile) -> Result<Self, Self::Error> {
        let worlds_path = resolve_worlds_path(config.worlds_path)?;
        let current_world_path = check_current_world_path(config.current_world_path)?;
        let server_socket_path = resolve_server_socket_path(config.server_socket_path)?;
        let users_file_path = resolve_users_file_path(config.users_file_path)?;
        let base_url = check_base_url(config.base_url)?;
        let min_password_length = config.min_password_length.into();
        let max_password_length = config.max_password_length.into();

        Ok(Self {
            listen_on: config.listen_on,
            app_config: AppConfig {
                worlds_path,
                current_world_path,
                server_socket_path,
                users_file_path,
                base_url,
                min_password_length,
                max_password_length,
            },
        })
    }
}

fn resolve_worlds_path(worlds_path: path::PathBuf) -> Result<path::PathBuf, ConfigValidationError> {
    let worlds_path = relative_path_to_absolute(worlds_path)
        .map_err(|err| ConfigValidationError::WorldsPath(err.to_string()))?;

    if !worlds_path.is_dir() {
        Err(ConfigValidationError::WorldsPath(format!(
            "`{}` must be a directory",
            worlds_path.display()
        )))
    } else {
        Ok(worlds_path)
    }
}

fn check_current_world_path(
    current_world: path::PathBuf,
) -> Result<path::PathBuf, ConfigValidationError> {
    if !current_world.is_symlink() {
        Err(ConfigValidationError::CurrentWorldName(format!(
            "`{}` must be a symlink",
            current_world.display(),
        )))
    } else {
        Ok(current_world)
    }
}

fn resolve_server_socket_path(
    server_socket_path: path::PathBuf,
) -> Result<path::PathBuf, ConfigValidationError> {
    let server_socket_path = relative_path_to_absolute(server_socket_path)
        .map_err(|err| ConfigValidationError::ServerSocketPath(err.to_string()))?;

    let fs_metadata = server_socket_path.metadata().map_err(|err| {
        ConfigValidationError::ServerSocketPath(format!(
            "Failed to read metadata for server socket file: {}",
            err
        ))
    })?;

    if !fs_metadata.file_type().is_socket() {
        Err(ConfigValidationError::ServerSocketPath(format!(
            "`{}` must be a socket",
            server_socket_path.display()
        )))
    } else {
        Ok(server_socket_path)
    }
}

fn resolve_users_file_path(
    users_file: path::PathBuf,
) -> Result<path::PathBuf, ConfigValidationError> {
    let users_file = relative_path_to_absolute(users_file)
        .map_err(|err| ConfigValidationError::UsersFilePath(err.to_string()))?;

    if !users_file.is_file() {
        Err(ConfigValidationError::UsersFilePath(format!(
            "`{}` must be a valid file",
            users_file.display()
        )))
    } else {
        Ok(users_file)
    }
}

fn check_base_url(url: uri::Uri) -> Result<uri::Uri, ConfigValidationError> {
    if url.scheme().is_none() {
        Err(ConfigValidationError::InvalidBaseUrl(url))
    } else {
        Ok(url)
    }
}

fn relative_path_to_absolute<P: AsRef<path::Path>>(
    path: P,
) -> Result<path::PathBuf, LoadConfigError> {
    let path = path.as_ref();

    let path = if path.starts_with("./") || path.starts_with("../") {
        env::current_dir()
            .map_err(LoadConfigError::CurrentWorkingDir)?
            .join(path)
    } else if path.has_root() {
        path.to_owned()
    } else {
        env::current_exe()
            .map_err(LoadConfigError::ExecutablePath)?
            .with_file_name(path)
    };

    fs::canonicalize(&path).map_err(|source| LoadConfigError::CanonicalizePath { path, source })
}
