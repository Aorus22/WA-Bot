use crate::views::chat::*;
use crate::views::chat::media::voice_recorder::{format_timer, VoiceRecorder, MAX_SECONDS, MIN_SECONDS};

impl ChatView {
    /// The mic button: open the microphone and switch the composer to its
    /// recording bar. The device opens in the background, so the bar shows
    /// at once and the clock starts when the microphone answers.
    pub(crate) fn start_voice_recording(&mut self, cx: &mut Context<Self>) {
        let Some(chat_id) = self.selected_chat_id.clone() else { return };
        if self.voice.is_some() {
            return;
        }
        self.voice_seq = self.voice_seq.wrapping_add(1);
        let seq = self.voice_seq;
        self.voice = Some(VoiceDraft { seq, chat_id, recorder: None });
        self.attach_panel = AttachPanel::None;

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let res = TOKIO_RT.spawn_blocking(VoiceRecorder::start).await;
                let _ = cx_handle.update(|cx: &mut App| {
                    let Some(view) = this.upgrade() else { return };
                    view.update(cx, |this, cx| {
                        // Cancelled (or another recording started) while the device opened:
                        // a recorder that arrives late is dropped, which stops it.
                        let current = this.voice.as_ref().is_some_and(|draft| draft.seq == seq);
                        match res {
                            Ok(Ok(recorder)) if current => {
                                if let Some(draft) = this.voice.as_mut() {
                                    draft.recorder = Some(recorder);
                                }
                                this.tick_voice_clock(seq, cx);
                            }
                            Ok(Err(message)) if current => {
                                this.voice = None;
                                toast::error(message, cx);
                            }
                            Err(_) if current => {
                                this.voice = None;
                                toast::error("Perekaman gagal dimulai", cx);
                            }
                            _ => {}
                        }
                        cx.notify();
                    });
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Redraw a few times a second while recording `seq`, so the clock moves.
    fn tick_voice_clock(&mut self, seq: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
            let alive = this
                .update(cx, |this, cx| {
                    let alive = this.voice.as_ref().is_some_and(|draft| draft.seq == seq);
                    if alive {
                        cx.notify();
                    }
                    alive
                })
                .unwrap_or(false);
            if !alive {
                break;
            }
        })
        .detach();
    }

    /// The bin: throw the recording away.
    pub(crate) fn cancel_voice_recording(&mut self, cx: &mut Context<Self>) {
        // Dropping the draft closes the microphone.
        self.voice = None;
        cx.notify();
    }

    /// Stop recording and send the note: a pending bubble at once, the WAV
    /// uploaded as a push-to-talk message (the backend transcodes it).
    pub(crate) fn send_voice_recording(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.voice.as_mut() else { return };
        // Still opening the microphone: nothing to send yet.
        let Some(recorder) = draft.recorder.take() else { return };
        let chat_id = draft.chat_id.clone();
        self.voice = None;
        let base_url = Self::compose_base_url(cx);
        cx.notify();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx_handle = cx.clone();
            async move {
                let Ok(recording) = TOKIO_RT.spawn_blocking(move || recorder.finish()).await else {
                    let _ = cx_handle.update(|cx: &mut App| toast::error("Perekaman gagal disimpan", cx));
                    return;
                };
                if recording.seconds < MIN_SECONDS {
                    let _ = cx_handle.update(|cx: &mut App| toast::info("Rekaman terlalu pendek", cx));
                    return;
                }
                if recording.peak == 0 {
                    let _ = cx_handle.update(|cx: &mut App| {
                        toast::error("Mikrofon tidak menangkap suara. Periksa perangkat dan izin mikrofon.", cx)
                    });
                    return;
                }

                // The pending bubble goes into the chat on screen; if the user has
                // moved on, the note still goes out and shows up when they return.
                let target = chat_id.clone();
                let temp_id = cx_handle.update(|cx: &mut App| {
                    let view = this.upgrade()?;
                    view.update(cx, |this, cx| {
                        (this.selected_chat_id.as_deref() == Some(target.as_str()))
                            .then(|| this.push_optimistic_attachment(cx, &target, "[Audio]".to_string(), "ptt", None))
                    })
                });

                let client = HttpClient::new(&base_url);
                let (target, seconds, wav) = (chat_id.clone(), recording.seconds as f64, recording.wav);
                let res = TOKIO_RT
                    .spawn(async move {
                        client.send_audio(&target, wav, "voice-note.wav", true, Some(seconds), None).await
                    })
                    .await;

                let _ = cx_handle.update(|cx: &mut App| {
                    let Some(view) = this.upgrade() else { return };
                    view.update(cx, |this, cx| {
                        let sent = match res {
                            Ok(Ok(id_res)) => Some(id_res.id),
                            _ => None,
                        };
                        let failed = sent.is_none();
                        if let Some(temp_id) = temp_id.as_deref() {
                            this.finish_optimistic_attachment(cx, &chat_id, temp_id, sent);
                        }
                        if failed {
                            toast::error("Gagal mengirim pesan suara", cx);
                        }
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }

    /// The composer's recording bar: bin, pulsing red dot, clock, send.
    pub(crate) fn render_voice_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        let draft = self.voice.as_ref()?;
        let theme = *cx.app_theme();
        let seconds = draft.recorder.as_ref().map_or(0.0, |r| r.seconds()).min(MAX_SECONDS as f32);
        let ready = draft.recorder.is_some();
        let recording_red: Hsla = rgb(0xef4444).into();
        Some(
            h_flex().p_3().child(
                h_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .px_2()
                    .py_1()
                    .rounded_3xl()
                    .border_1()
                    .border_color(theme.border.opacity(0.2))
                    .bg(theme.muted.opacity(0.5))
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .id("btn-voice-cancel")
                            .p_2()
                            .rounded(px(cx.shape().radius_button))
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_shrink_0()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.muted.opacity(0.65)))
                            .tooltip(move |window, cx| Tooltip::new("Discard recording").build(window, cx))
                            .child(svg().data(TRASH_SVG).size(px(20.0)).text_color(theme.muted_foreground))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.cancel_voice_recording(cx);
                            })),
                    )
                    .child(
                        div()
                            .w(px(10.0))
                            .h(px(10.0))
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(recording_red)
                            .with_animation(
                                "voice-recording-dot",
                                gpui::Animation::new(std::time::Duration::from_millis(1100)).repeat(),
                                move |el, d| el.opacity(0.35 + 0.65 * (d * std::f32::consts::TAU).sin().abs()),
                            ),
                    )
                    .child(
                        div()
                            .min_w(px(40.0))
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(format_timer(seconds)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(if ready { "Recording…" } else { "Starting microphone…" }),
                    )
                    .child(
                        div()
                            .id("btn-voice-send")
                            .p_2()
                            .rounded(px(cx.shape().radius_button))
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_shrink_0()
                            .bg(theme.primary.opacity(if ready { 1.0 } else { 0.5 }))
                            .when(ready, |el| el.cursor_pointer().hover(|h| h.opacity(0.85)))
                            .tooltip(move |window, cx| Tooltip::new("Send voice message").build(window, cx))
                            .child(svg().data(SEND_SVG).size(px(18.0)).text_color(theme.primary_foreground))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.send_voice_recording(cx);
                            })),
                    ),
            ),
        )
    }
}
