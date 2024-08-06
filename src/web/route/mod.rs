mod enroll;
mod index;
mod login;

pub use enroll::{get as enroll_get, post as enroll_post};
pub use index::get as index_get;
pub use login::{get as login_get, post as login_post};
