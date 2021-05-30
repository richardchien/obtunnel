pub const MAGIC_PREFIX: &'static str = "(>_<)...";
pub const MAX_MSG_LEN: usize = 4500;
pub const MTU: usize = (MAX_MSG_LEN - MAGIC_PREFIX.len()) * 3 / 4;

pub const ONEBOT_WS_URL: &'static str = "ws://127.0.0.1:6701/";
