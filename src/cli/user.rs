use crate::core;

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

    let mut url = config.base_url;
    url.set_path("/enroll");
    url.set_query(Some(&format!("token={}", token.reveal())));

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
