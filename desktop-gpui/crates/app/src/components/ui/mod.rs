//! GPUI's shadcn-style app UI kit.
//!
//! Each component has its own module so views can depend on a stable app-facing
//! path. Components with app theme requirements are implemented locally;
//! others adapt the corresponding `gpui-component` control.

pub mod accordion;
pub mod alert;
pub mod alert_dialog;
pub mod aspect_ratio;
pub mod avatar;
pub mod badge;
pub mod boxed_list;
pub mod breadcrumb;
pub mod button;
pub mod button_group;
pub mod calendar;
pub mod card;
pub mod carousel;
pub mod chart;
pub mod checkbox;
pub mod collapsible;
pub mod combobox;
pub mod command;
pub mod context_menu;
pub mod dialog;
pub mod drawer;
pub mod dropdown_menu;
pub mod empty;
pub mod field;
pub mod form;
pub mod form_row;
pub mod hover_card;
pub mod input;
pub mod input_group;
pub mod input_otp;
pub mod item;
pub mod kbd;
pub mod label;
pub mod menubar;
pub mod native_select;
pub mod pagination;
pub mod popover;
pub mod progress;
pub mod radio_group;
pub mod resizable;
pub mod scroll_area;
pub mod select;
pub mod separator;
pub mod sheet;
pub mod sidebar;
pub mod skeleton;
pub mod slider;
pub mod sonner;
pub mod spinner;
pub mod switch;
pub mod table;
pub mod tabs;
pub mod text_field;
pub mod textarea;
pub mod toggle;
pub mod toggle_group;
pub mod tooltip;

// Common controls stay available at `components::ui::X` for concise view code.
pub use alert::{Alert, AlertVariant};
pub use avatar::{Avatar, AvatarGroup};
pub use badge::Badge;
pub use boxed_list::boxed_list;
pub use button::{
    Button, ButtonVariant, ButtonVariants, action_button, button, destructive_button, icon_button,
};
pub use card::{card, card_group, settings_group};
pub use checkbox::Checkbox;
pub use form_row::form_row;
pub use input::{Input, InputEvent, InputState};
pub use label::{HighlightsMatch, Label};
pub use progress::{Progress, ProgressCircle};
pub use radio_group::{Radio, RadioGroup};
pub use scroll_area::{Scrollbar, ScrollbarMode};
pub use select::{Select, SelectState};
pub use separator::{Separator, SeparatorStyle};
pub use slider::{Slider, SliderEvent, SliderState, SliderValue};
pub use spinner::Spinner;
pub use switch::Switch;
pub use tabs::{Tab, TabBar, TabVariant};
pub use text_field::{TextArea, TextField, TextFieldEvent};
pub use textarea::{Textarea, TextareaState};
pub use tooltip::Tooltip;
pub use gpui_component::{Disableable, Icon, IconName, Size, Sizable, WindowExt, h_flex, v_flex};
