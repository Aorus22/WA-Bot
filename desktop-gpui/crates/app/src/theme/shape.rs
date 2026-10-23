//! Shape tokens: radii and surface depth, the counterpart of the colour
//! tokens in [`super::manager::ActiveTokens`]. The "GTK style" setting swaps
//! between two sets; views read them through [`ShapeExt::shape`] instead of
//! hard-coding `.rounded_xl()` so one switch restyles everything that opted in.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextRoleTokens {
    pub caption_size: f32,
    pub body_size: f32,
    pub heading_size: f32,
    pub title4_size: f32,
    pub title3_size: f32,
    pub title2_size: f32,
    pub caption_opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeTokens {
    pub typography: TextRoleTokens,
    /// Buttons, inputs, small rows.
    pub radius_control: f32,
    /// Cards, popovers, list rows.
    pub radius_card: f32,
    /// Flat selectable list rows.
    pub radius_row: f32,
    /// Navigation rail items.
    pub radius_nav_item: f32,
    /// Icon buttons: a circle by default, a rounded square in the GTK style.
    pub radius_button: f32,
    /// Selected segment radius inside a toggle group.
    pub radius_toggle_selected: f32,
    /// The message composer field.
    pub radius_input: f32,
    /// Search and settings text fields.
    pub radius_field: f32,
    /// Height of the header rows (chat list, conversation, sheets).
    pub header_height: f32,
    /// `gpui-component` widgets (buttons, inputs, switches, sliders, popovers).
    pub widget_radius: f32,
    /// `gpui-component` dialogs and notifications.
    pub widget_radius_lg: f32,
    /// Grouped setting rows and similar boxed lists.
    pub radius_group: f32,
    /// Flat corners used by interior boxed-list rows.
    pub radius_flat: f32,
    /// Boxed-list separator thickness.
    pub list_separator_size: f32,
    /// Boxed-list row minimum height and padding.
    pub list_row_min_height: f32,
    pub list_row_padding_x: f32,
    pub list_row_padding_y: f32,
    /// Message bubbles.
    pub radius_bubble: f32,
    /// The tight tail corner on a message bubble.
    pub radius_bubble_tail: f32,
    /// Soft drop shadows under bubbles and cards.
    pub shadows: bool,
    /// Dialog and popover radius.
    pub radius_popover: f32,
    /// Native window corner radius.
    pub radius_window: f32,
    /// Application titlebar height.
    pub titlebar_height: f32,
    /// Adwaita icon-button and regular button dimension.
    pub control_size: f32,
    /// Visible face of the window controls; the surrounding hit area stays wider.
    pub titlebar_control_size: f32,
    /// Scrollbar thickness.
    pub scrollbar_width: f32,
    /// Base spacing unit.
    pub spacing_unit: f32,
    /// Adwaita state transition duration.
    pub transition_ms: u64,
    /// Whether this token set is the Adwaita set.
    pub adwaita: bool,
}

impl ShapeTokens {
    /// The look before the GTK style existed (Tailwind `md`/`xl`/`2xl`).
    pub const DEFAULT: Self = Self {
        typography: TextRoleTokens {
            caption_size: 0.75,
            body_size: 1.0,
            heading_size: 0.875,
            title4_size: 1.125,
            title3_size: 1.25,
            title2_size: 1.5,
            caption_opacity: 0.7,
        },
        widget_radius: 6.0,
        widget_radius_lg: 8.0,
        radius_group: 8.0,
        radius_flat: 0.0,
        list_separator_size: 0.0,
        list_row_min_height: 0.0,
        list_row_padding_x: 0.0,
        list_row_padding_y: 0.0,
        radius_control: 6.0,
        radius_card: 12.0,
        radius_row: 12.0,
        radius_nav_item: 16.0,
        radius_button: 9999.0,
        radius_toggle_selected: 9999.0,
        radius_input: 24.0,
        radius_field: 12.0,
        header_height: 52.0,
        radius_bubble: 16.0,
        radius_bubble_tail: 4.0,
        shadows: true,
        radius_popover: 14.0,
        radius_window: 15.0,
        titlebar_height: 48.0,
        control_size: 32.0,
        titlebar_control_size: 32.0,
        scrollbar_width: 6.0,
        spacing_unit: 8.0,
        transition_ms: 0,
        adwaita: false,
    };

    /// libadwaita (values from discord-gnome-theme): 9px buttons, 12px cards,
    /// 46px headerbar, flat surfaces.
    pub const ADWAITA: Self = Self {
        typography: TextRoleTokens {
            caption_size: 0.82,
            body_size: 1.0,
            heading_size: 1.0,
            title4_size: 1.18,
            title3_size: 1.36,
            title2_size: 1.36,
            caption_opacity: 0.5,
        },
        widget_radius: 9.0,
        widget_radius_lg: 15.0,
        radius_group: 12.0,
        radius_flat: 0.0,
        list_separator_size: 1.0,
        list_row_min_height: 54.0,
        list_row_padding_x: 16.0,
        list_row_padding_y: 10.0,
        radius_control: 9.0,
        radius_card: 12.0,
        radius_row: 6.0,
        radius_nav_item: 9.0,
        radius_button: 9.0,
        radius_toggle_selected: 6.0,
        radius_input: 9.0,
        radius_field: 9.0,
        header_height: 46.0,
        radius_bubble: 12.0,
        radius_bubble_tail: 4.0,
        shadows: false,
        radius_popover: 15.0,
        radius_window: 15.0,
        titlebar_height: 46.0,
        control_size: 34.0,
        titlebar_control_size: 26.0,
        scrollbar_width: 8.0,
        spacing_unit: 6.0,
        transition_ms: 200,
        adwaita: true,
    };

    pub fn new(gtk_style: bool) -> Self {
        if gtk_style {
            Self::ADWAITA
        } else {
            Self::DEFAULT
        }
    }
}

impl Default for ShapeTokens {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pub trait ShapeExt {
    fn shape(&self) -> ShapeTokens;
}

impl ShapeExt for gpui::App {
    fn shape(&self) -> ShapeTokens {
        super::manager::ThemeManager::global(self).shape
    }
}

impl<T> ShapeExt for gpui::Context<'_, T> {
    fn shape(&self) -> ShapeTokens {
        super::manager::ThemeManager::global(self).shape
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtk_style_picks_adwaita() {
        assert_eq!(ShapeTokens::new(false), ShapeTokens::DEFAULT);
        assert_eq!(ShapeTokens::new(true), ShapeTokens::ADWAITA);
        assert!(!ShapeTokens::ADWAITA.shadows);
        assert_eq!(ShapeTokens::ADWAITA.widget_radius_lg, 15.0);
        assert_eq!(ShapeTokens::ADWAITA.typography.caption_size, 0.82);
        assert_eq!(ShapeTokens::ADWAITA.typography.title2_size, 1.36);
    }
}
