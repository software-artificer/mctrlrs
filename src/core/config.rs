use super::properties;
use std::{env, fs, io, net, num, path};

#[derive(serde::Deserialize)]
struct ConfigFile {
    listen_on: net::SocketAddr,
    worlds_path: path::PathBuf,
    users_file_path: path::PathBuf,
    base_url: url::Url,
    #[serde(default = "default_min_password_len")]
    min_password_length: u8,
    #[serde(default = "default_max_password_len")]
    max_password_length: u8,
    server_properties_path: path::PathBuf,
    tls_key: Option<path::PathBuf>,
    tls_chain: Option<path::PathBuf>,
    worker_count: Option<num::NonZeroUsize>,
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
    ParseFailure(#[from] serde_yaml_ng::Error),
    #[error("Failed to read configuration file contents {}", .path.display())]
    ReadError {
        path: path::PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Failed to canonicalize the path {}: {source}", .path.display())]
    CanonicalizePath {
        path: path::PathBuf,
        source: io::Error,
    },
    #[error("Failed to validate configuration file: {0}")]
    Validate(#[from] ConfigValidationError),
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigValidationError {
    #[error("Invalid worlds path: {0}")]
    WorldsPath(String),
    #[error("Invalid users file path: {0}")]
    UsersFilePath(String),
    #[error("Invalid base URL: {0}")]
    InvalidBaseUrl(url::Url),
    #[error("Invalid server.properties path: {}", .0.display())]
    PropertiesPath(path::PathBuf),
    #[error("Unable to load server.properties file: {0}")]
    LoadProperties(#[source] properties::Error),
    #[error("Invalid TLS configuration: {0}")]
    Tls(String),
}

pub struct AppConfig {
    pub worlds_path: path::PathBuf,
    pub rcon_address: net::SocketAddr,
    pub users_file_path: path::PathBuf,
    pub base_url: url::Url,
    pub min_password_length: usize,
    pub max_password_length: usize,
    pub server_properties_path: path::PathBuf,
    pub rcon_password: secrecy::SecretString,
}

pub struct TlsConfig {
    pub key: path::PathBuf,
    pub chain: path::PathBuf,
}

pub struct Config {
    pub listen_on: net::SocketAddr,
    pub app_config: AppConfig,
    pub tls: Option<TlsConfig>,
    pub worker_count: Option<num::NonZeroUsize>,
}

impl Config {
    pub fn load<P: AsRef<path::Path>>(path: P) -> Result<Self, LoadConfigError> {
        let path = canonicalize_path(path)?;
        let config_reader =
            fs::File::open(&path).map_err(|source| LoadConfigError::ReadError { path, source })?;
        let config: ConfigFile =
            serde_yaml_ng::from_reader(config_reader).map_err(LoadConfigError::ParseFailure)?;

        config.try_into().map_err(LoadConfigError::Validate)
    }
}

impl TryFrom<ConfigFile> for Config {
    type Error = ConfigValidationError;

    fn try_from(config: ConfigFile) -> Result<Self, Self::Error> {
        let worlds_path = resolve_worlds_path(config.worlds_path)?;
        let users_file_path = resolve_users_file_path(config.users_file_path)?;
        let base_url = check_base_url(config.base_url)?;
        let min_password_length = config.min_password_length.into();
        let max_password_length = config.max_password_length.into();
        let server_properties_path =
            resolve_server_properties_file_path(config.server_properties_path)?;
        let rcon_properties = load_server_properties(&server_properties_path)?;
        let tls = resolve_tls_config(config.tls_key, config.tls_chain)?;

        Ok(Self {
            listen_on: config.listen_on,
            tls,
            app_config: AppConfig {
                worlds_path,
                users_file_path,
                base_url,
                min_password_length,
                max_password_length,
                server_properties_path,
                rcon_address: net::SocketAddr::from((
                    net::Ipv4Addr::new(127, 0, 0, 1),
                    rcon_properties.port,
                )),
                rcon_password: rcon_properties.password,
            },
            worker_count: config.worker_count,
        })
    }
}

fn resolve_tls_config(
    key: Option<path::PathBuf>,
    chain: Option<path::PathBuf>,
) -> Result<Option<TlsConfig>, ConfigValidationError> {
    match (key, chain) {
        (Some(key), Some(chain)) => Ok(Some(TlsConfig { key, chain })),
        (None, None) => Ok(None),
        _ => Err(ConfigValidationError::Tls(
            "Both `tls_key` and `tls_chain` options need to be either present or absent"
                .to_string(),
        ))?,
    }
}

fn load_server_properties(
    path: &path::Path,
) -> Result<properties::RconProperties, ConfigValidationError> {
    let properties =
        properties::Properties::parse(path).map_err(ConfigValidationError::LoadProperties)?;
    let rcon_properties = properties
        .rcon_properties()
        .map_err(ConfigValidationError::LoadProperties)?;

    Ok(rcon_properties)
}

fn resolve_worlds_path(worlds_path: path::PathBuf) -> Result<path::PathBuf, ConfigValidationError> {
    let worlds_path = canonicalize_path(worlds_path)
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

fn resolve_server_properties_file_path(
    properties_path: path::PathBuf,
) -> Result<path::PathBuf, ConfigValidationError> {
    canonicalize_path(&properties_path)
        .map_err(|_| ConfigValidationError::PropertiesPath(properties_path))
}

fn resolve_users_file_path(
    users_file: path::PathBuf,
) -> Result<path::PathBuf, ConfigValidationError> {
    let users_file = canonicalize_path(users_file)
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

fn check_base_url(url: url::Url) -> Result<url::Url, ConfigValidationError> {
    if url.scheme().starts_with("http") {
        Ok(url)
    } else {
        Err(ConfigValidationError::InvalidBaseUrl(url))
    }
}

fn canonicalize_path<P: AsRef<path::Path>>(path: P) -> Result<path::PathBuf, LoadConfigError> {
    let path = relative_path_to_absolute(path)?;

    fs::canonicalize(&path).map_err(|source| LoadConfigError::CanonicalizePath { path, source })
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
    Ok(path)
}
