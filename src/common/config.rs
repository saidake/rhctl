#[derive(Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub sudo: bool,
    pub rsync: bool,
    pub silent: bool,
}