use crate::core;
use argon2::password_hash::{self, rand_core::OsRng, PasswordHasher, SaltString};
use rand::distributions::{self, DistString};
use secrecy::ExposeSecret;
use std::{collections, fmt, fs, io, path};

trait SafeString {
    fn is_safe(&self) -> bool;
}

impl SafeString for String {
    fn is_safe(&self) -> bool {
        self.chars()
            .all(|c| char::is_ascii_alphanumeric(&c) || c == '_')
    }
}

#[derive(thiserror::Error, Debug)]
pub enum InvalidUsernameError {
    #[error("The username can not be longer than {} characters.", 0)]
    TooLong(usize),
    #[error("The username can not be empty.")]
    TooShort,
    #[error(r#"Username "{}" contains invalid characters. Allowed characters are letters "a" to "z", digits "0" to "9" and the underscore "_" character."#, 0)]
    InvalidCharacters(String),
}

#[derive(Clone)]
pub struct Username(String);

impl Username {
    const MAX_USERNAME_LENGTH: usize = 64;
}

impl TryFrom<String> for Username {
    type Error = InvalidUsernameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(InvalidUsernameError::TooShort)
        } else if value.len() > Self::MAX_USERNAME_LENGTH {
            Err(InvalidUsernameError::TooLong(Self::MAX_USERNAME_LENGTH))
        } else if !value.is_safe() {
            Err(InvalidUsernameError::InvalidCharacters(value))
        } else {
            Ok(Username(value))
        }
    }
}

impl fmt::Display for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(thiserror::Error, Debug)]
#[error("Invalid token format.")]
pub struct InvalidTokenError;

#[derive(Clone)]
pub struct EnrollToken(secrecy::SecretString);

impl PartialEq for EnrollToken {
    fn eq(&self, other: &Self) -> bool {
        self.0.expose_secret() == other.0.expose_secret()
    }
}

impl EnrollToken {
    const TOKEN_LENGTH: usize = 128;

    pub fn reveal(&self) -> &str {
        self.0.expose_secret()
    }
}

impl TryFrom<String> for EnrollToken {
    type Error = InvalidTokenError;

    fn try_from(token: String) -> Result<Self, Self::Error> {
        if token.is_safe() || token.len() != Self::TOKEN_LENGTH {
            Ok(Self(secrecy::Secret::new(token)))
        } else {
            Err(InvalidTokenError)
        }
    }
}

impl TryFrom<&str> for EnrollToken {
    type Error = InvalidTokenError;

    fn try_from(token: &str) -> Result<Self, Self::Error> {
        token.to_string().try_into()
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct UserRecord {
    username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    enroll_token: Option<String>,
}

pub struct User {
    pub username: Username,
    password: Option<secrecy::SecretString>,
    enroll_token: Option<EnrollToken>,
}

pub struct Users {
    users: collections::HashMap<String, User>,
    storage_path: path::PathBuf,
}

impl Users {
    pub fn load<P: AsRef<path::Path>>(path: P) -> Result<Self, ManageUsersError> {
        let storage_path = path.as_ref().to_owned();

        let users_file = fs::File::open(&storage_path).map_err(ManageUsersError::LoadStorage)?;
        let users: Vec<UserRecord> =
            serde_yml::from_reader(users_file).map_err(ManageUsersError::Deserialize)?;
        let users = parse_users(users)?;

        Ok(Self {
            users,
            storage_path,
        })
    }

    pub fn enroll_user(mut self, username: Username) -> Result<EnrollToken, ManageUsersError> {
        let password = None;
        let enroll_token: EnrollToken = {
            let mut rng = rand::thread_rng();
            let token_string =
                distributions::Alphanumeric.sample_string(&mut rng, EnrollToken::TOKEN_LENGTH);
            token_string
                .try_into()
                .map_err(ManageUsersError::GenerateToken)?
        };

        let enroll_user_token = enroll_token.clone();
        let enroll_token = enroll_token;

        self.users.insert(
            username.to_string(),
            User {
                username,
                password,
                enroll_token: Some(enroll_user_token),
            },
        );

        self.persist()?;

        Ok(enroll_token)
    }

    pub fn remove(mut self, username: &Username) -> Result<(), ManageUsersError> {
        if self.users.remove(&username.0).is_some() {
            self.persist()
        } else {
            Err(ManageUsersError::NoSuchUser(username.0.clone()))
        }
    }

    pub fn find_username_by_token(&self, token: EnrollToken) -> Option<Username> {
        self.users
            .values()
            .find(|user| user.enroll_token.as_ref() == Some(&token))
            .map(|user| user.username.to_owned())
    }

    pub fn update_password(
        mut self,
        username: &Username,
        password: Password,
    ) -> Result<(), ManageUsersError> {
        match self.users.get_mut(&username.to_string()) {
            Some(user) => {
                user.password = Some(password.0);
                user.enroll_token = None;

                self.persist()
            }
            None => Err(ManageUsersError::NoSuchUser(username.to_string())),
        }
    }

    fn persist(self) -> Result<(), ManageUsersError> {
        let storage_file = fs::File::create(&self.storage_path)
            .map_err(|err| ManageUsersError::Persist(err.to_string()))?;
        let user_records: Vec<UserRecord> = self.into();
        serde_yml::to_writer(storage_file, &user_records)
            .map_err(|err| ManageUsersError::Persist(err.to_string()))?;

        Ok(())
    }
}

impl TryFrom<UserRecord> for User {
    type Error = String;

    fn try_from(user_record: UserRecord) -> Result<Self, String> {
        let username = user_record
            .username
            .try_into()
            .map_err(|err: InvalidUsernameError| err.to_string())?;

        if user_record.password.is_some() && user_record.enroll_token.is_some() {
            Err(format!(
                "User `{}` has both a password and an enroll token set.",
                username
            ))
        } else if user_record.password.is_none() && user_record.enroll_token.is_none() {
            Err(format!(
                "User `{}` has no password or an enroll token set.",
                username
            ))
        } else {
            let enroll_token = match user_record.enroll_token {
                Some(token) => {
                    let token = token.try_into().map_err(|err| {
                        format!("User `{}` has invalid enroll token: {}", username, err)
                    })?;

                    Some(token)
                }
                _ => None,
            };

            Ok(Self {
                username,
                password: user_record.password.map(secrecy::Secret::new),
                enroll_token,
            })
        }
    }
}

impl From<Users> for Vec<UserRecord> {
    fn from(users: Users) -> Self {
        users
            .users
            .into_values()
            .map(|user| UserRecord {
                username: user.username.to_string(),
                password: user.password.map(|pass| pass.expose_secret().clone()),
                enroll_token: user
                    .enroll_token
                    .map(|token| token.0.expose_secret().clone()),
            })
            .collect()
    }
}

fn parse_users(
    users: Vec<UserRecord>,
) -> Result<collections::HashMap<String, User>, ManageUsersError> {
    users
        .into_iter()
        .map(|user| {
            user.try_into()
                .map_err(ManageUsersError::CorruptStorage)
                .map(|user: User| (user.username.to_string(), user))
        })
        .collect()
}

#[derive(thiserror::Error, Debug)]
pub enum ManageUsersError {
    #[error("Failed to load users from storage: {}", .0)]
    LoadStorage(#[source] io::Error),
    #[error("Storage corruption detected: {}", .0)]
    CorruptStorage(String),
    #[error("Failed to deserialize storage data: {}", .0)]
    Deserialize(#[source] serde_yml::Error),
    #[error("Failed to generate enroll token: {}", .0)]
    GenerateToken(#[from] InvalidTokenError),
    #[error("Failed to persist users data: {}", .0)]
    Persist(String),
    #[error("User not found: {}", .0)]
    NoSuchUser(String),
}

pub enum PasswordError {
    Short(usize),
    Long(usize),
    Weak,
    Hash(password_hash::Error),
}

pub struct Password(secrecy::SecretString);

impl Password {
    pub fn new(password: String, config: &core::AppConfig) -> Result<Self, PasswordError> {
        if password.len() < config.min_password_length {
            Err(PasswordError::Short(config.min_password_length))
        } else if password.len() > config.max_password_length {
            Err(PasswordError::Long(config.max_password_length))
        } else if !is_strong_password(&password) {
            Err(PasswordError::Weak)
        } else {
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = argon2::Argon2::default();
            let password_hash = argon2
                .hash_password(password.as_bytes(), &salt)
                .map_err(PasswordError::Hash)?;

            Ok(Self(secrecy::Secret::new(password_hash.to_string())))
        }
    }
}

fn is_strong_password(password: &str) -> bool {
    let mut lowercase = 0;
    let mut uppercase = 0;
    let mut digit = 0;
    let mut punctuation = 0;

    for ch in password.chars() {
        if ch.is_ascii_lowercase() {
            lowercase = 1;
        }

        if ch.is_ascii_uppercase() {
            uppercase = 1;
        }

        if ch.is_ascii_digit() {
            digit = 1;
        }

        if ch.is_ascii_punctuation() {
            punctuation = 1;
        }

        if lowercase + uppercase + digit + punctuation >= 3 {
            return true;
        }
    }

    false
}
