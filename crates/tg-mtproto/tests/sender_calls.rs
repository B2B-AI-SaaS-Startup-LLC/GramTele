use tg_mtproto::{
    mtp_instance::{Fields, Instance, Mode},
    mtproto_dc_options::Environment,
    sender::Sender,
};
#[test]
fn five_real_client_request_shapes_compile_and_keep_response_types() {
    let instance = Instance::new(Mode::Normal, Fields::new(Environment::Test));
    let api = Sender::new(instance.clone());
    // config_loader.cpp: help.getConfig.
    let first = api
        .request(tg_scheme::HelpGetConfig::new())
        .done(|result: tg_scheme::Config| {
            let _ = result.c_config().dc_options.len();
        })
        .send();
    // api_cloud_password.cpp: account.getPassword.
    api.request(tg_scheme::AccountGetPassword::new())
        .done(|_: tg_scheme::AccountPassword| {})
        .fail(|_| {})
        .send();
    // api_chat_filters.cpp: ImportInvite.
    api.request(tg_scheme::ChatlistsJoinChatlistInvite::new(
        b"slug".to_vec(),
        vec![tg_scheme::mtp_input_peer_self()],
    ))
    .done(|_: tg_scheme::Updates| {})
    .fail(|_| {})
    .handle_flood_errors()
    .send();
    // dedicated_file_loader.cpp: upload.getFile on an updater DC.
    api.request(tg_scheme::UploadGetFile::new(
        tg_tl::Flags::default(),
        tg_scheme::mtp_input_document_file_location(1, 2, vec![], vec![]),
        0,
        1024 * 1024,
    ))
    .to_dc(tg_mtproto::facade::updater_dc_id(2))
    .done(|_: tg_scheme::UploadFile| {})
    .fail_with_response(|_, _| {})
    .send();
    // api_peer_photo.cpp: EditChatPhoto after history->sendRequestId.
    api.request(tg_scheme::MessagesEditChatPhoto::new(
        1,
        tg_scheme::mtp_input_chat_photo_empty(),
    ))
    .done(|_: tg_scheme::Updates| {})
    .fail(|_| {})
    .after_request(first)
    .send();
    assert!(api.pending(first));
    drop(api);
    assert!(!tg_mtproto::session::SessionEnvironment::has_callback(
        &instance, first
    ));
    instance.shutdown();
}
