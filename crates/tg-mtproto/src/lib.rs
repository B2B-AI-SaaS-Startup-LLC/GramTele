//! Transport and authentication-key layer of AyuGram MTProto (below sessions).
#![forbid(unsafe_code)]
pub mod config_loader;
pub mod connection_abstract;
pub mod connection_http;
pub mod connection_resolving;
pub mod connection_tcp;
pub mod core_types;
pub mod dedicated_file_loader;
pub mod details;
pub mod facade;
pub mod mtp_instance;
pub mod mtproto_auth_key;
pub mod mtproto_concurrent_sender;
pub mod mtproto_config;
pub mod mtproto_dc_options;
pub mod mtproto_dh_utils;
pub mod mtproto_proxy_data;
pub mod mtproto_response;
pub mod proxy_check;
pub mod sender;
pub mod session;
pub mod session_private;
pub mod special_config_request;
pub mod type_utils;
