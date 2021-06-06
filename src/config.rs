// TODO: use TOML
pub const MAGIC_PREFIX: &'static str = "(>_<)...";
pub const MAX_MSG_LEN: usize = 4500;
pub const MTU: usize = (MAX_MSG_LEN - MAGIC_PREFIX.len()) * 3 / 4;
