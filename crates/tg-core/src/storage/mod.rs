pub mod details;
pub mod serialize_common;
pub mod storage_account;
pub mod storage_cloud_blob;
pub mod storage_domain;
pub mod storage_facade;
pub mod storage_file_lock;
#[cfg(unix)]
pub mod storage_file_lock_posix;
#[cfg(windows)]
pub mod storage_file_lock_win;
pub mod storage_shared_media;
pub mod storage_sparse_ids_list;
pub mod storage_user_photos;
