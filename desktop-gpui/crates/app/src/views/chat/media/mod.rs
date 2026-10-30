//! Chat-only media viewers and playback/capture support.

mod image_viewer;
pub(crate) mod thumbnail;
pub mod video_player;
mod video_viewer;
pub(crate) mod viewer_math;
pub mod voice_recorder;

pub(crate) use video_viewer::VideoView;
