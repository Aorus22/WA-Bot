use reqwest::multipart::{Form, Part};
use crate::client::HttpClient;
use crate::dto::{GroupCache, IdResult};
use crate::error::{ClientError, Result};

#[derive(Debug, Clone)]
pub struct SendMediaOptions {
    pub ptt: Option<bool>,
    pub seconds: Option<f64>,
    pub waveform: Option<String>,
    pub view_once: Option<bool>,
    pub secret: Option<String>,
}

impl Default for SendMediaOptions {
    fn default() -> Self {
        Self {
            ptt: None,
            seconds: None,
            waveform: None,
            view_once: None,
            secret: None,
        }
    }
}

pub struct SendMediaBuilder {
    target: String,
    media_type: String,
    caption: String,
    file_bytes: Vec<u8>,
    file_name: String,
    mime_type: Option<String>,
    options: SendMediaOptions,
}

impl SendMediaBuilder {
    pub fn new(
        target: impl Into<String>,
        media_type: impl Into<String>,
        file_bytes: Vec<u8>,
        file_name: impl Into<String>,
    ) -> Self {
        Self {
            target: target.into(),
            media_type: media_type.into(),
            caption: String::new(),
            file_bytes,
            file_name: file_name.into(),
            mime_type: None,
            options: SendMediaOptions::default(),
        }
    }

    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }

    pub fn mime_type(mut self, mime: impl Into<String>) -> Self {
        self.mime_type = Some(mime.into());
        self
    }

    pub fn options(mut self, options: SendMediaOptions) -> Self {
        self.options = options;
        self
    }

    pub fn ptt(mut self, ptt: bool) -> Self {
        self.options.ptt = Some(ptt);
        self
    }

    pub fn seconds(mut self, seconds: f64) -> Self {
        self.options.seconds = Some(seconds);
        self
    }

    pub fn waveform(mut self, waveform: impl Into<String>) -> Self {
        self.options.waveform = Some(waveform.into());
        self
    }

    pub fn view_once(mut self, view_once: bool) -> Self {
        self.options.view_once = Some(view_once);
        self
    }

    pub fn build_form(self) -> Form {
        let bytes = self.file_bytes;
        let name = self.file_name;
        let part = Part::bytes(bytes.clone()).file_name(name.clone());
        let part = if let Some(ref mime) = self.mime_type {
            part.mime_str(mime).unwrap_or_else(|_| Part::bytes(bytes).file_name(name))
        } else {
            part
        };

        let secret = self
            .options
            .secret
            .or_else(|| std::env::var("API_SECRET").ok())
            .unwrap_or_else(|| "default-secret".to_string());

        let mut form = Form::new()
            .text("secret", secret)
            .text("target", self.target)
            .text("message", self.caption)
            .text("type", self.media_type)
            .part("file", part);

        if let Some(ptt) = self.options.ptt {
            form = form.text("ptt", ptt.to_string());
        }
        if let Some(sec) = self.options.seconds {
            form = form.text("seconds", sec.to_string());
        }
        if let Some(wf) = self.options.waveform {
            form = form.text("waveform", wf);
        }
        if let Some(vo) = self.options.view_once {
            if vo {
                form = form.text("viewOnce", "true");
            }
        }

        form
    }
}

impl HttpClient {
    pub async fn send_media(&self, builder: SendMediaBuilder) -> Result<IdResult> {
        let url = format!("{}/send-media", self.base_url());
        let form = builder.build_form();
        let resp = self.client().post(url).multipart(form).send().await?;
        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            return Err(ClientError::Api {
                status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                message: error_text,
            });
        }
        let parsed = resp.json::<IdResult>().await?;
        Ok(parsed)
    }

    pub async fn send_audio(
        &self,
        target: &str,
        file_bytes: Vec<u8>,
        file_name: &str,
        ptt: bool,
        seconds: Option<f64>,
        waveform: Option<String>,
    ) -> Result<IdResult> {
        let media_type = if ptt { "ptt" } else { "audio" };
        let mut builder = SendMediaBuilder::new(target, media_type, file_bytes, file_name)
            .ptt(ptt);
        if let Some(s) = seconds {
            builder = builder.seconds(s);
        }
        if let Some(w) = waveform {
            builder = builder.waveform(w);
        }
        self.send_media(builder).await
    }

    pub async fn set_group_photo(
        &self,
        group_id: &str,
        image_bytes: Vec<u8>,
        file_name: &str,
    ) -> Result<GroupCache> {
        let url = format!("{}/groups/{}/photo", self.base_url(), group_id);
        let part = Part::bytes(image_bytes).file_name(file_name.to_string());
        let form = Form::new().part("file", part);
        let resp = self.client().post(url).multipart(form).send().await?;
        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            return Err(ClientError::Api {
                status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                message: error_text,
            });
        }
        let parsed = resp.json::<GroupCache>().await?;
        Ok(parsed)
    }

    pub async fn post_status_media(
        &self,
        media_type: &str,
        file_bytes: Vec<u8>,
        file_name: &str,
        caption: Option<&str>,
    ) -> Result<IdResult> {
        let url = format!("{}/statuses/media", self.base_url());
        let part = Part::bytes(file_bytes).file_name(file_name.to_string());
        let mut form = Form::new()
            .text("type", media_type.to_string())
            .part("file", part);
        if let Some(c) = caption {
            form = form.text("caption", c.to_string());
        }
        let resp = self.client().post(url).multipart(form).send().await?;
        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            return Err(ClientError::Api {
                status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                message: error_text,
            });
        }
        let parsed = resp.json::<IdResult>().await?;
        Ok(parsed)
    }
}
