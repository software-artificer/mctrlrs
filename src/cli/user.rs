use crate::core;
use actix_web::http::uri;
use std::str::FromStr;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid username")]
    InvalidUserName(#[from] core::InvalidUsernameError),
    #[error("Failed to enroll the user: {}", .0)]
    FailedToEnrol(#[source] core::ManageUsersError),
    #[error("Failed to remove the user: {}", .0)]
    FailedToDelete(#[source] core::ManageUsersError),
}

pub fn enroll(config: core::AppConfig, username: String) -> Result<(), Error> {
    let username: core::Username = username.try_into()?;

    let users = core::Users::load(config.users_file_path).map_err(Error::FailedToEnrol)?;
    let token = users.enroll_user(username).map_err(Error::FailedToEnrol)?;

    let mut parts = config.base_url.into_parts();
    let path_and_query = uri::PathAndQuery::from_str(&format!("/enroll?token={}", token.reveal()))
        .expect("Failed to create the path and query part for an enrollment URL.");
    parts.path_and_query = Some(path_and_query);
    let url: uri::Uri = uri::Uri::from_parts(parts).expect("Failed to generate an enrollment URL.");

    println!("To finish the enrollment visit {}", url);

    Ok(())
}

pub fn remove(config: core::AppConfig, username: String) -> Result<(), Error> {
    let username: core::Username = username.try_into()?;

    let users = core::Users::load(config.users_file_path).map_err(Error::FailedToDelete)?;
    users.remove(&username).map_err(Error::FailedToDelete)?;

    println!("User {} was successfully removed", username);

    Ok(())
}
