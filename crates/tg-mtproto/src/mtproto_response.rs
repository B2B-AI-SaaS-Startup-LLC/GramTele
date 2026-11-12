//! RPC error parsing, preserving Qt regex options and local error conventions.
use tg_base::qthelp_regex::{regex_match, RegExOptions};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: i32,
    pub kind: String,
    pub description: String,
}
impl Error {
    pub fn from_rpc_error(error: &tg_scheme::RpcError) -> Self {
        let data = error.c_rpc_error();
        let text = String::from_utf8_lossy(&data.error_message);
        let m = regex_match(
            "^([A-Z0-9_]+)(: .*)?$",
            &text,
            RegExOptions::DOT_MATCHES_EVERYTHING | RegExOptions::MULTILINE,
        );
        let (kind, description) = if m.has_match() {
            (
                m.captured(1).into(),
                m.captured(2).get(2..).unwrap_or_default().into(),
            )
        } else if data.error_code < 0 || data.error_code >= 500 {
            ("INTERNAL_SERVER_ERROR".into(), text.into_owned())
        } else {
            (
                "CLIENT_BAD_RPC_ERROR".into(),
                format!("Bad rpc error received, text = '{text}'"),
            )
        };
        Self {
            code: data.error_code,
            kind,
            description,
        }
    }
    pub fn from_reply(reply: &[i32]) -> Self {
        let mut reader = tg_tl::Reader::new(reply);
        match <tg_scheme::RpcError as tg_tl::Deserialize>::read(&mut reader) {
            Ok(error) => Self::from_rpc_error(&error),
            Err(_) => Self::local("RESPONSE_PARSE_FAILED", "Error parse failed."),
        }
    }
    pub fn mtp_local(kind: &str, description: &str) -> tg_scheme::RpcError {
        tg_scheme::mtp_rpc_error(
            0,
            format!(
                "CLIENT_{kind}{}",
                if description.is_empty() {
                    String::new()
                } else {
                    format!(": {description}")
                }
            )
            .into_bytes(),
        )
    }
    pub fn local(kind: &str, description: &str) -> Self {
        Self::from_rpc_error(&Self::mtp_local(kind, description))
    }
    pub fn is_flood_error(&self) -> bool {
        is_flood_error(&self.kind)
    }
    pub fn is_temporary_error(&self) -> bool {
        self.code < 0 || self.code >= 500 || self.is_flood_error()
    }
    pub fn is_default_handled_error(&self) -> bool {
        self.is_temporary_error()
    }
    pub fn ignore_error(&self) -> bool {
        self.code == 406
    }
}
pub fn is_flood_error(kind: &str) -> bool {
    kind.starts_with("FLOOD_WAIT_") || kind.starts_with("FLOOD_PREMIUM_WAIT_")
}
#[derive(Clone, Default, Debug)]
pub struct Response {
    pub reply: tg_tl::Buffer,
    pub outer_msg_id: u64,
    pub request_id: i32,
}
pub type DoneHandler = Box<dyn FnMut(&Response) -> bool + Send>;
pub type FailHandler = Box<dyn Fn(&Error, &Response) -> bool + Send>;
#[derive(Default)]
pub struct ResponseHandler {
    pub done: Option<DoneHandler>,
    pub fail: Option<FailHandler>,
}

pub const NO_ERROR: i32 = 0;
pub const TIMEOUT_ERROR: i32 = 1;
/// Window-independent part of ShowErrorFallback; the caller supplies showToast.
pub fn show_error_fallback(show_toast: impl FnOnce(&str), error: &Error) {
    if !error.ignore_error() {
        show_toast(&error.kind);
    }
}
