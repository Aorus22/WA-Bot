use wabot_backend_client::multipart::{SendMediaBuilder, SendMediaOptions};

#[test]
fn test_send_media_builder_basic() {
    let builder = SendMediaBuilder::new("12345@s.whatsapp.net", "image", vec![1, 2, 3], "test.jpg")
        .caption("Hello caption");
    let _form = builder.build_form();
}

#[test]
fn test_send_media_builder_audio_voice_note() {
    let builder = SendMediaBuilder::new("12345@s.whatsapp.net", "ptt", vec![4, 5, 6], "voice.ogg")
        .ptt(true)
        .seconds(5.2)
        .waveform("AQIDBA==")
        .view_once(true);
    let _form = builder.build_form();
}

#[test]
fn test_send_media_options_custom_secret() {
    let mut opts = SendMediaOptions::default();
    opts.secret = Some("my-secret-key".to_string());
    let builder = SendMediaBuilder::new("12345@s.whatsapp.net", "document", vec![7, 8], "doc.pdf")
        .options(opts);
    let _form = builder.build_form();
}
