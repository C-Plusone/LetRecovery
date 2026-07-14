use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{COLORREF, HANDLE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_DONOTROUND, DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreatePen, CreateSolidBrush,
    DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect, GetWindowDC, InvalidateRect, LineTo,
    MoveToEx, RedrawWindow, ReleaseDC, RoundRect, SelectObject, SetBkMode, SetTextColor,
    DT_END_ELLIPSIS, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, HBRUSH, HDC, PAINTSTRUCT, PEN_STYLE,
    RDW_FRAME, RDW_INVALIDATE, RDW_NOERASE, SRCCOPY, TRANSPARENT,
};
use windows::Win32::UI::Controls::{
    GetComboBoxInfo, SetWindowTheme, CDDS_ITEMPREPAINT, CDDS_PREPAINT, CDRF_DODEFAULT,
    CDRF_NOTIFYITEMDRAW, CDRF_SKIPDEFAULT, CDRF_SKIPPOSTPAINT, COMBOBOXINFO, HDITEMW, HDI_TEXT,
    LVIF_TEXT, LVITEMW, NMLVCUSTOMDRAW, NM_CUSTOMDRAW,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, IsWindowEnabled, TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetClientRect, GetParent, GetPropW, GetWindowLongPtrW, GetWindowRect,
    RemovePropW, SendMessageW, SetPropW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WM_CAPTURECHANGED,
    WM_ENABLE, WM_ERASEBKGND, WM_GETFONT, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCDESTROY, WM_NCPAINT, WM_NOTIFY, WM_PAINT, WM_SETFOCUS,
    WM_SIZE, WM_THEMECHANGED, WS_BORDER, WS_EX_CLIENTEDGE,
};
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use super::controls::{
    button_visual, draw_antialiased_control_frame, draw_progress, fill_round_rect_antialiased,
    rounded_control_frame_geometry, ButtonRole, ControlState, ProgressRole,
};

const fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    COLORREF((red as u32) | ((green as u32) << 8) | ((blue as u32) << 16))
}

/// Inno Setup 6.7 `WizardStyle=modern ... windows11` color roles.
/// Values are taken from the fixed-reference screenshots and its Windows 11 VCL styles.
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub window: COLORREF,
    pub nav: COLORREF,
    pub edit: COLORREF,
    pub button: COLORREF,
    pub button_hot: COLORREF,
    pub button_pressed: COLORREF,
    pub text: COLORREF,
    pub text_secondary: COLORREF,
    pub text_disabled: COLORREF,
    pub border: COLORREF,
    pub separator: COLORREF,
    pub accent_fill: COLORREF,
    pub accent_border: COLORREF,
    /// Inno Modern Windows 11 task progress and checked-state accent.
    pub progress: COLORREF,
}

impl Palette {
    pub const LIGHT: Self = Self {
        dark: false,
        window: rgb(249, 249, 249),
        nav: rgb(249, 249, 249),
        edit: rgb(255, 255, 255),
        button: rgb(253, 253, 253),
        button_hot: rgb(249, 249, 249),
        button_pressed: rgb(233, 233, 233),
        text: rgb(0, 0, 0),
        text_secondary: rgb(59, 59, 59),
        text_disabled: rgb(157, 157, 157),
        border: rgb(230, 230, 230),
        separator: rgb(222, 222, 222),
        accent_fill: rgb(0, 95, 184),
        accent_border: rgb(0, 96, 184),
        progress: rgb(113, 199, 132),
    };

    pub const DARK: Self = Self {
        dark: true,
        window: rgb(43, 43, 43),
        nav: rgb(43, 43, 43),
        edit: rgb(31, 31, 31),
        button: rgb(55, 55, 55),
        button_hot: rgb(62, 62, 62),
        button_pressed: rgb(47, 47, 47),
        text: rgb(255, 255, 255),
        text_secondary: rgb(214, 214, 214),
        text_disabled: rgb(120, 120, 120),
        border: rgb(67, 67, 67),
        separator: rgb(81, 81, 81),
        accent_fill: rgb(49, 72, 83),
        accent_border: rgb(66, 149, 192),
        progress: rgb(113, 199, 132),
    };

    pub fn system() -> Self {
        #[cfg(feature = "non-elevated-tests")]
        match std::env::var("LETRECOVERY_UI_THEME").as_deref() {
            Ok("dark") => return Self::DARK,
            Ok("light") => return Self::LIGHT,
            _ => {}
        }

        let personalization = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
            .ok();
        let apps_use_light: Option<u32> = personalization
            .as_ref()
            .and_then(|key| key.get_value("AppsUseLightTheme").ok());
        if apps_use_light == Some(0) {
            Self::DARK
        } else {
            Self::LIGHT
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeControlKind {
    General,
    Field,
    ScrollableField,
    List,
    ListView,
    Header,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeThemeClass {
    Explorer,
    DarkExplorer,
    Cfd,
    DarkCfd,
    ItemsView,
    DarkItemsView,
}

const fn native_theme_class(kind: NativeControlKind, dark: bool) -> NativeThemeClass {
    match (kind, dark) {
        (NativeControlKind::Header, false) => NativeThemeClass::ItemsView,
        (NativeControlKind::Header, true) => NativeThemeClass::DarkItemsView,
        (NativeControlKind::Field, false) => NativeThemeClass::Cfd,
        (NativeControlKind::Field, true) => NativeThemeClass::DarkCfd,
        (
            NativeControlKind::General
            | NativeControlKind::ScrollableField
            | NativeControlKind::List
            | NativeControlKind::ListView,
            true,
        ) => NativeThemeClass::DarkExplorer,
        _ => NativeThemeClass::Explorer,
    }
}

/// Applies the native theme class that covers both the control client area and its non-client
/// scrollbar. Multiline edits deliberately use Explorer rather than CFD in dark mode because the
/// latter leaves a light Win32 scrollbar on several supported Windows builds.
///
/// Field frames (Edit / ComboBox / ListBox) use the host Windows 11 visual styles only. A previous
/// owner-drawn rounded overlay left residual system-accent “blue feet” at the four rectangular
/// corners and fought the Fluent control chrome, so it is no longer installed on those HWNDs.
pub unsafe fn apply_control_theme(control: HWND, palette: Palette, kind: NativeControlKind) {
    let class = match native_theme_class(kind, palette.dark) {
        NativeThemeClass::Explorer => w!("Explorer"),
        NativeThemeClass::DarkExplorer => w!("DarkMode_Explorer"),
        NativeThemeClass::Cfd => w!("CFD"),
        NativeThemeClass::DarkCfd => w!("DarkMode_CFD"),
        NativeThemeClass::ItemsView => w!("ItemsView"),
        NativeThemeClass::DarkItemsView => w!("DarkMode_ItemsView"),
    };
    let _ = SetWindowTheme(control, class, PCWSTR::null());
    let class_name = control_class_name(control);
    let is_edit = is_edit_class(&class_name);
    let is_combo = is_combo_class(&class_name);
    if palette.dark && is_auto_radio_button(&class_name, GetWindowLongPtrW(control, GWL_STYLE)) {
        // The undocumented DarkMode_Explorer radio renderer still paints its caption black on
        // several Windows 11 builds.  With visual-style painting disabled for this one control
        // type, the normal WM_CTLCOLORBTN path owns the caption and therefore uses the dialog's
        // verified light-on-dark palette; auto-radio grouping and keyboard behaviour are intact.
        // Empty strings disable themed painting for this HWND. Passing null would merely reset
        // it to the process theme and lets the same black-caption renderer come back.
        let _ = SetWindowTheme(control, w!(""), w!(""));
        let _ = InvalidateRect(control, None, true);
    }
    if is_edit && is_single_line_edit(control) {
        // Match the real Windows 11 property-page Edit supplied by the user:
        // style 0x50010080 has no WS_BORDER, while ex-style 0x00000204 includes
        // WS_EX_CLIENTEDGE.  The v6 theme then owns the recessed surface and focus underline.
        apply_property_page_edit_style(control);
        let _ = RemoveWindowSubclass(
            control,
            Some(rounded_control_subclass),
            ROUNDED_CONTROL_SUBCLASS_ID,
        );
    } else if is_edit {
        apply_single_border_style(control);
    } else if is_combo && matches!(kind, NativeControlKind::Field) {
        apply_borderless_style(control);
        let _ = SetWindowSubclass(
            control,
            Some(rounded_control_subclass),
            ROUNDED_CONTROL_SUBCLASS_ID,
            usize::from(palette.dark),
        );
        let _ = InvalidateRect(control, None, false);
    } else if matches!(kind, NativeControlKind::List) && is_list_box(control) {
        // Standalone ListBoxes retain their existing Inno row palette, but the HWND itself has one
        // clipped, borderless surface so USER32 cannot expose square blue/black corner pixels.
        apply_borderless_style(control);
        let _ = SetWindowSubclass(
            control,
            Some(rounded_control_subclass),
            ROUNDED_CONTROL_SUBCLASS_ID,
            usize::from(palette.dark),
        );
        let _ = InvalidateRect(control, None, false);
    } else if matches!(
        kind,
        NativeControlKind::Field | NativeControlKind::ScrollableField | NativeControlKind::List
    ) {
        // ComboBox and other field HWNDs: drop any earlier rounded-frame subclass so only the
        // Windows 11 themed border remains.
        let _ = RemoveWindowSubclass(
            control,
            Some(rounded_control_subclass),
            ROUNDED_CONTROL_SUBCLASS_ID,
        );
        let _ = InvalidateRect(control, None, false);
    }

    // A native ComboBox owns a separate top-level ComboLBox window. The list is not covered by
    // theming the ComboBox HWND itself, which otherwise leaves a white popup in dark mode.
    let mut info = COMBOBOXINFO {
        cbSize: std::mem::size_of::<COMBOBOXINFO>() as u32,
        ..Default::default()
    };
    if GetComboBoxInfo(control, &mut info).is_ok() && !info.hwndList.0.is_null() {
        // The popup is a ListBox, not another field frame.  DarkMode_CFD is correct for the
        // closed ComboBox but corrupts the popup/arrow painting on some Windows 11 builds (the
        // selected string is drawn a second time in the arrow area).  Explorer keeps the popup
        // dark without changing the closed ComboBox renderer.
        let popup_class = if palette.dark {
            w!("DarkMode_Explorer")
        } else {
            w!("Explorer")
        };
        let _ = SetWindowTheme(info.hwndList, popup_class, PCWSTR::null());
        // Inno's TNewComboBox is a stock TComboBox. Preserve USER32's normal rectangular popup
        // renderer; this removes the slower WM_DRAWITEM/rounded-overlay paths and keeps keyboard,
        // hover and accessibility behaviour identical to the native control.
        let _ = RemoveWindowSubclass(
            info.hwndList,
            Some(rounded_control_subclass),
            ROUNDED_CONTROL_SUBCLASS_ID,
        );
        let _ = RemovePropW(info.hwndList, LIST_BOX_HOT_PROPERTY);
        apply_combo_popup_native_chrome(info.hwndList, palette);
        let _ = InvalidateRect(info.hwndList, None, false);
    }
}

unsafe fn apply_combo_popup_native_chrome(popup: HWND, palette: Palette) {
    // ComboLBox is a separate top-level HWND and does not inherit dark mode from the owner.
    // Explicitly opt out of DWM rounding: the user requested the stock rectangular Windows popup,
    // while its client rows, keyboard navigation and accessibility remain entirely USER32-owned.
    let corner_preference = DWMWCP_DONOTROUND;
    let _ = DwmSetWindowAttribute(
        popup,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        (&corner_preference as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
        std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
    );
    let immersive_dark = if palette.dark { 1i32 } else { 0i32 };
    let _ = DwmSetWindowAttribute(
        popup,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        (&immersive_dark as *const i32).cast(),
        std::mem::size_of_val(&immersive_dark) as u32,
    );
}

unsafe fn control_class_name(control: HWND) -> String {
    let mut buffer = [0u16; 64];
    let length = GetClassNameW(control, &mut buffer);
    String::from_utf16_lossy(&buffer[..usize::try_from(length.max(0)).unwrap_or(0)])
}

fn is_edit_class(class_name: &str) -> bool {
    class_name.eq_ignore_ascii_case("Edit")
}

fn is_combo_class(class_name: &str) -> bool {
    class_name.eq_ignore_ascii_case("ComboBox")
}

const fn button_style_is_auto_radio(style: isize) -> bool {
    const BUTTON_TYPE_MASK: isize = 0x000f;
    const BS_AUTORADIOBUTTON_VALUE: isize = 0x0009;
    style & BUTTON_TYPE_MASK == BS_AUTORADIOBUTTON_VALUE
}

fn is_auto_radio_button(class_name: &str, style: isize) -> bool {
    class_name.eq_ignore_ascii_case("Button") && button_style_is_auto_radio(style)
}

unsafe fn is_single_line_edit(control: HWND) -> bool {
    const ES_MULTILINE: isize = 0x0004;
    GetWindowLongPtrW(control, GWL_STYLE) & ES_MULTILINE == 0
}

fn borderless_style_bits(style: isize, ex_style: isize) -> (isize, isize) {
    (
        style & !(WS_BORDER.0 as isize),
        ex_style & !(WS_EX_CLIENTEDGE.0 as isize),
    )
}

fn property_page_edit_style_bits(style: isize, ex_style: isize) -> (isize, isize) {
    const WS_EX_NOPARENTNOTIFY_VALUE: isize = 0x0000_0004;
    (
        style & !(WS_BORDER.0 as isize),
        ex_style | WS_EX_CLIENTEDGE.0 as isize | WS_EX_NOPARENTNOTIFY_VALUE,
    )
}

fn single_border_style_bits(style: isize, ex_style: isize) -> (isize, isize) {
    (
        style | WS_BORDER.0 as isize,
        ex_style & !(WS_EX_CLIENTEDGE.0 as isize),
    )
}

unsafe fn apply_borderless_style(control: HWND) {
    let style = GetWindowLongPtrW(control, GWL_STYLE);
    let ex_style = GetWindowLongPtrW(control, GWL_EXSTYLE);
    let (style, ex_style) = borderless_style_bits(style, ex_style);
    apply_control_frame_styles(control, style, ex_style);
}

unsafe fn apply_property_page_edit_style(control: HWND) {
    let style = GetWindowLongPtrW(control, GWL_STYLE);
    let ex_style = GetWindowLongPtrW(control, GWL_EXSTYLE);
    let (style, ex_style) = property_page_edit_style_bits(style, ex_style);
    apply_control_frame_styles(control, style, ex_style);
}

unsafe fn apply_single_border_style(control: HWND) {
    let style = GetWindowLongPtrW(control, GWL_STYLE);
    let ex_style = GetWindowLongPtrW(control, GWL_EXSTYLE);
    let (style, ex_style) = single_border_style_bits(style, ex_style);
    apply_control_frame_styles(control, style, ex_style);
}

unsafe fn apply_control_frame_styles(control: HWND, style: isize, ex_style: isize) {
    let current_style = GetWindowLongPtrW(control, GWL_STYLE);
    let current_ex_style = GetWindowLongPtrW(control, GWL_EXSTYLE);
    if current_style == style && current_ex_style == ex_style {
        return;
    }
    let _ = SetWindowLongPtrW(control, GWL_STYLE, style);
    let _ = SetWindowLongPtrW(control, GWL_EXSTYLE, ex_style);
    let _ = SetWindowPos(
        control,
        None,
        0,
        0,
        0,
        0,
        SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
    let _ = InvalidateRect(control, None, false);
}

/// Themes both halves of a report ListView. The header is a separate HWND and otherwise retains a
/// light background even when the list client colors are explicitly dark.
pub unsafe fn apply_list_view_theme(list: HWND, palette: Palette) -> Option<HWND> {
    // USER32/comctl32 can repaint a square non-client border after hover, focus or header paint.
    // Keep the report implementation native, but reserve its outer frame for the deterministic
    // antialiased overlay installed below.
    apply_borderless_style(list);
    apply_control_theme(list, palette, NativeControlKind::ListView);
    // Do not make every caller remember the three independent ListView colour messages.  In
    // particular, an empty report has no item custom-draw callback and otherwise exposes the
    // class default white body in dark mode.
    set_list_view_colors(list, palette);
    let _ = InvalidateRect(list, None, false);
    let _ = SetWindowSubclass(
        list,
        Some(list_view_subclass),
        LIST_VIEW_SUBCLASS_ID,
        usize::from(palette.dark),
    );
    // Selection colour is delivered as NM_CUSTOMDRAW to the ListView parent rather than the
    // ListView itself. Install one keyed parent subclass per list, so dialogs containing two
    // reports remain independent and a real selected row uses the Inno highlighted-button fill.
    if let Ok(parent) = GetParent(list) {
        let list_value = list.0 as usize;
        let dark_flag = usize::from(palette.dark) << (usize::BITS - 1);
        let _ = SetWindowSubclass(
            parent,
            Some(list_view_parent_subclass),
            LIST_VIEW_PARENT_SUBCLASS_ID ^ list_value,
            list_value | dark_flag,
        );
    }
    let header = SendMessageW(list, 0x101F, WPARAM(0), LPARAM(0)); // LVM_GETHEADER
    if header.0 == 0 {
        return None;
    }
    let header = HWND(header.0 as *mut _);
    apply_control_theme(header, palette, NativeControlKind::Header);
    // A report header is parented to the ListView itself, so HDF_OWNERDRAW sends WM_DRAWITEM to
    // the ListView instead of our dialog content window.  Subclassing the header is the only
    // deterministic way to avoid dark ItemsView drawing black text on a black header.
    let _ = SetWindowSubclass(
        header,
        Some(header_subclass),
        HEADER_SUBCLASS_ID,
        usize::from(palette.dark),
    );
    let _ = InvalidateRect(header, None, false);
    Some(header)
}

unsafe fn set_list_view_colors(list: HWND, palette: Palette) {
    for (message, color) in [
        (0x1001, palette.edit), // LVM_SETBKCOLOR
        (0x1026, palette.edit), // LVM_SETTEXTBKCOLOR
        (0x1024, palette.text), // LVM_SETTEXTCOLOR
    ] {
        let _ = SendMessageW(list, message, WPARAM(0), LPARAM(color.0 as isize));
    }
}

/// Applies a deterministic Inno-style paint path to the one native progress control still used by
/// a tool dialog.  UxTheme's progress class ignores the app dark mode and otherwise leaves a light
/// trough in the partition-copy window.
pub unsafe fn apply_progress_theme(control: HWND, palette: Palette) {
    let _ = SetWindowTheme(control, PCWSTR::null(), PCWSTR::null());
    let _ = SetWindowSubclass(
        control,
        Some(progress_subclass),
        PROGRESS_SUBCLASS_ID,
        usize::from(palette.dark),
    );
    let _ = InvalidateRect(control, None, false);
}

/// Applies a deterministic dark/light paint path to the horizontal target-size trackbar.  The
/// standard trackbar theme has no supported dark variant and paints a nearly white channel/thumb.
pub unsafe fn apply_trackbar_theme(control: HWND, palette: Palette) {
    let _ = SetWindowTheme(control, PCWSTR::null(), PCWSTR::null());
    let _ = SetWindowSubclass(
        control,
        Some(trackbar_subclass),
        TRACKBAR_SUBCLASS_ID,
        usize::from(palette.dark),
    );
    let _ = InvalidateRect(control, None, false);
}

const HEADER_SUBCLASS_ID: usize = 0x4c52_4844;
const LIST_VIEW_SUBCLASS_ID: usize = 0x4c52_4c56;
const LIST_VIEW_PARENT_SUBCLASS_ID: usize = 0x4c52_4c50;
const PROGRESS_SUBCLASS_ID: usize = 0x4c52_5052;
const TRACKBAR_SUBCLASS_ID: usize = 0x4c52_5442;
const ROUNDED_CONTROL_SUBCLASS_ID: usize = 0x4c52_5243;
const LIST_BOX_HOT_PROPERTY: PCWSTR = w!("LetRecovery.InnoListBox.HotItem");
const ROUNDED_CONTROL_HOT_PROPERTY: PCWSTR = w!("LetRecovery.InnoControl.Hot");
const WM_MOUSELEAVE_MESSAGE: u32 = 0x02a3;
const WM_NCMOUSEMOVE_MESSAGE: u32 = 0x00a0;
const WM_NCMOUSELEAVE_MESSAGE: u32 = 0x02a2;

const fn palette_from_reference(reference_data: usize) -> Palette {
    if reference_data != 0 {
        Palette::DARK
    } else {
        Palette::LIGHT
    }
}

unsafe fn redraw_control_frame(control: HWND) {
    let _ = RedrawWindow(
        control,
        None,
        None,
        RDW_FRAME | RDW_INVALIDATE | RDW_NOERASE,
    );
}

unsafe fn ensure_hot_tracking(hwnd: HWND, property: PCWSTR, non_client: bool) {
    if !GetPropW(hwnd, property).is_invalid() {
        return;
    }
    let mut tracking = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE
            | if non_client {
                TME_NONCLIENT
            } else {
                Default::default()
            },
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    if TrackMouseEvent(&mut tracking).is_ok()
        && SetPropW(hwnd, property, HANDLE(std::ptr::dangling_mut())).is_ok()
    {
        redraw_control_frame(hwnd);
    }
}

unsafe fn clear_hot_tracking(hwnd: HWND, property: PCWSTR) {
    if RemovePropW(hwnd, property).is_ok_and(|handle| !handle.is_invalid()) {
        redraw_control_frame(hwnd);
    }
}

unsafe extern "system" fn header_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    reference_data: usize,
) -> LRESULT {
    match message {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            paint_header(hwnd, palette_from_reference(reference_data));
            LRESULT(0)
        }
        WM_THEMECHANGED => {
            let _ = InvalidateRect(hwnd, None, false);
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, Some(header_subclass), HEADER_SUBCLASS_ID);
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, message, wparam, lparam),
    }
}

unsafe fn paint_header(hwnd: HWND, palette: Palette) {
    let mut paint = PAINTSTRUCT::default();
    let dc = BeginPaint(hwnd, &mut paint);
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    fill(dc, &client, palette.button);
    let font = SendMessageW(hwnd, WM_GETFONT, WPARAM(0), LPARAM(0));
    let old_font = if font.0 != 0 {
        Some(SelectObject(
            dc,
            windows::Win32::Graphics::Gdi::HGDIOBJ(font.0 as *mut _),
        ))
    } else {
        None
    };
    let _ = SetBkMode(dc, TRANSPARENT);
    let _ = SetTextColor(dc, palette.text);
    let dpi = GetDpiForWindow(hwnd).max(96);
    let inset = scale(8, dpi);
    let count = SendMessageW(hwnd, 0x1200, WPARAM(0), LPARAM(0)).0 as i32; // HDM_GETITEMCOUNT
    for index in 0..count.max(0) {
        let mut rect = RECT::default();
        if SendMessageW(
            hwnd,
            0x1207, // HDM_GETITEMRECT
            WPARAM(index as usize),
            LPARAM((&mut rect as *mut RECT) as isize),
        )
        .0 == 0
        {
            continue;
        }
        let mut text = vec![0u16; 256];
        let mut item = HDITEMW {
            mask: HDI_TEXT,
            pszText: windows::core::PWSTR(text.as_mut_ptr()),
            cchTextMax: text.len() as i32,
            ..Default::default()
        };
        let _ = SendMessageW(
            hwnd,
            0x120B, // HDM_GETITEMW
            WPARAM(index as usize),
            LPARAM((&mut item as *mut HDITEMW) as isize),
        );
        text.truncate(
            text.iter()
                .position(|value| *value == 0)
                .unwrap_or(text.len()),
        );
        let mut text_rect = rect;
        text_rect.left += inset;
        text_rect.right -= inset.min((text_rect.right - text_rect.left).max(0));
        let _ = DrawTextW(
            dc,
            &mut text,
            &mut text_rect,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        let separator = RECT {
            left: rect.right - 1,
            top: rect.top + scale(4, dpi),
            right: rect.right,
            bottom: rect.bottom - scale(4, dpi),
        };
        fill(dc, &separator, palette.separator);
    }
    if let Some(old_font) = old_font {
        let _ = SelectObject(dc, old_font);
    }
    // The header is a child of the report and can repaint after the report itself (for example on
    // hover). Reapply the report's top corners in this same paint transaction so it cannot expose
    // a square header edge over the rounded list frame.
    if let Ok(list) = GetParent(hwnd) {
        draw_rounded_control_frame_to_dc(dc, list, palette, palette.button);
    }
    let _ = EndPaint(hwnd, &paint);
}

unsafe extern "system" fn list_view_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    reference_data: usize,
) -> LRESULT {
    match message {
        WM_ERASEBKGND => {
            // An empty or temporarily disabled report receives no item custom-draw callbacks.
            // Fill the complete client area here so loading never exposes comctl32's white class
            // brush before the first row exists.
            let mut client = RECT::default();
            let _ = GetClientRect(hwnd, &mut client);
            fill(
                HDC(wparam.0 as *mut _),
                &client,
                palette_from_reference(reference_data).edit,
            );
            LRESULT(1)
        }
        WM_PAINT => {
            // A report with no rows never reaches NM_CUSTOMDRAW.  Some themed/disabled
            // comctl32 paths repaint the empty body with the class brush after WM_ERASEBKGND,
            // undoing the LVM_SETBKCOLOR value and exposing a white loading rectangle.  Own the
            // complete empty paint transaction so there is no later default fill to overwrite it.
            let item_count = SendMessageW(hwnd, 0x1004, WPARAM(0), LPARAM(0)).0; // LVM_GETITEMCOUNT
            if list_view_needs_empty_body_paint(item_count) {
                let mut paint = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut paint);
                fill(
                    dc,
                    &paint.rcPaint,
                    palette_from_reference(reference_data).edit,
                );
                let _ = EndPaint(hwnd, &paint);
                paint_rounded_control_frame(hwnd, palette_from_reference(reference_data));
                return LRESULT(0);
            }
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            // Checkbox glyphs only — the list frame stays under the Windows 11 ItemsView /
            // Explorer theme so corners match other Fluent controls without blue residual feet.
            paint_list_view_checkboxes(hwnd, palette_from_reference(reference_data));
            paint_rounded_control_frame(hwnd, palette_from_reference(reference_data));
            result
        }
        WM_NCPAINT => {
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            paint_rounded_control_frame(hwnd, palette_from_reference(reference_data));
            result
        }
        WM_ENABLE | WM_SETFOCUS | WM_KILLFOCUS | WM_SIZE | WM_THEMECHANGED => {
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            // Comctl32 can restore class-default colours while changing enabled/theme state.
            // Reassert all three ListView colours together; partial updates cause a white empty
            // body or black text background until the next full refresh.
            set_list_view_colors(hwnd, palette_from_reference(reference_data));
            let _ = InvalidateRect(hwnd, None, false);
            result
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, Some(list_view_subclass), LIST_VIEW_SUBCLASS_ID);
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, message, wparam, lparam),
    }
}

const fn list_view_needs_empty_body_paint(item_count: isize) -> bool {
    item_count == 0
}

unsafe extern "system" fn rounded_control_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    reference_data: usize,
) -> LRESULT {
    const CB_SHOWDROPDOWN: u32 = 0x014f;
    match message {
        WM_PAINT => {
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            if is_list_box(hwnd) {
                paint_list_box_rows(hwnd, palette_from_reference(reference_data));
            }
            paint_rounded_control_frame(hwnd, palette_from_reference(reference_data));
            result
        }
        WM_NCPAINT => {
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            paint_rounded_control_frame(hwnd, palette_from_reference(reference_data));
            result
        }
        WM_MOUSEMOVE => {
            if is_list_box(hwnd) {
                update_list_box_hot_item(hwnd, lparam);
            }
            ensure_hot_tracking(hwnd, ROUNDED_CONTROL_HOT_PROPERTY, false);
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
        WM_NCMOUSEMOVE_MESSAGE => {
            ensure_hot_tracking(hwnd, ROUNDED_CONTROL_HOT_PROPERTY, true);
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
        WM_MOUSELEAVE_MESSAGE | WM_NCMOUSELEAVE_MESSAGE => {
            if is_list_box(hwnd) {
                clear_list_box_hot_item(hwnd);
            }
            clear_hot_tracking(hwnd, ROUNDED_CONTROL_HOT_PROPERTY);
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
        WM_ENABLE | WM_SETFOCUS | WM_KILLFOCUS | WM_SIZE | WM_THEMECHANGED => {
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            let _ = InvalidateRect(hwnd, None, false);
            result
        }
        CB_SHOWDROPDOWN if wparam.0 != 0 => {
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            let mut info = COMBOBOXINFO {
                cbSize: std::mem::size_of::<COMBOBOXINFO>() as u32,
                ..Default::default()
            };
            if GetComboBoxInfo(hwnd, &mut info).is_ok() && !info.hwndList.0.is_null() {
                apply_combo_popup_native_chrome(
                    info.hwndList,
                    palette_from_reference(reference_data),
                );
            }
            result
        }
        WM_NCDESTROY => {
            let _ = RemovePropW(hwnd, LIST_BOX_HOT_PROPERTY);
            let _ = RemovePropW(hwnd, ROUNDED_CONTROL_HOT_PROPERTY);
            let _ = RemoveWindowSubclass(
                hwnd,
                Some(rounded_control_subclass),
                ROUNDED_CONTROL_SUBCLASS_ID,
            );
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, message, wparam, lparam),
    }
}

unsafe fn paint_rounded_control_frame(hwnd: HWND, palette: Palette) {
    let dc = GetWindowDC(hwnd);
    if dc.0.is_null() {
        return;
    }
    let class_name = control_class_name(hwnd);
    let interior = if is_combo_class(&class_name) {
        palette.button
    } else {
        palette.edit
    };
    draw_rounded_control_frame_to_dc(dc, hwnd, palette, interior);
    let _ = ReleaseDC(hwnd, dc);
}

unsafe fn draw_rounded_control_frame_to_dc(
    dc: HDC,
    hwnd: HWND,
    palette: Palette,
    interior: COLORREF,
) {
    let mut window = RECT::default();
    if GetWindowRect(hwnd, &mut window).is_err() {
        return;
    }
    let rect = RECT {
        left: 0,
        top: 0,
        right: (window.right - window.left).max(0),
        bottom: (window.bottom - window.top).max(0),
    };
    let Some(geometry) =
        rounded_control_frame_geometry(rect.right, rect.bottom, GetDpiForWindow(hwnd).max(96))
    else {
        return;
    };
    let class_name = control_class_name(hwnd);
    let interactive_field = is_combo_class(&class_name);
    let hot = !GetPropW(hwnd, ROUNDED_CONTROL_HOT_PROPERTY).is_invalid();
    let focused = GetFocus() == hwnd;
    let border = if !IsWindowEnabled(hwnd).as_bool() {
        palette.border
    } else if interactive_field && focused {
        palette.accent_border
    } else if interactive_field && hot {
        palette.separator
    } else {
        palette.border
    };
    draw_antialiased_control_frame(dc, rect, geometry, interior, border, palette.window);
}

unsafe fn is_list_box(hwnd: HWND) -> bool {
    matches!(control_class_name(hwnd).as_str(), "ListBox" | "ComboLBox")
}

unsafe fn update_list_box_hot_item(hwnd: HWND, lparam: LPARAM) {
    const LB_ITEMFROMPOINT: u32 = 0x01a9;
    let packed = SendMessageW(hwnd, LB_ITEMFROMPOINT, WPARAM(0), lparam).0 as u32;
    let outside = packed >> 16 != 0;
    let hot = (!outside).then_some((packed & 0xffff) as usize);
    let previous = GetPropW(hwnd, LIST_BOX_HOT_PROPERTY);
    let previous = (!previous.is_invalid()).then_some(previous.0 as usize - 1);
    if hot == previous {
        return;
    }
    let _ = RemovePropW(hwnd, LIST_BOX_HOT_PROPERTY);
    if let Some(index) = hot {
        let _ = SetPropW(
            hwnd,
            LIST_BOX_HOT_PROPERTY,
            HANDLE((index + 1) as *mut core::ffi::c_void),
        );
        let mut tracking = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: 0,
        };
        let _ = TrackMouseEvent(&mut tracking);
    }
    let _ = InvalidateRect(hwnd, None, false);
}

unsafe fn clear_list_box_hot_item(hwnd: HWND) {
    if RemovePropW(hwnd, LIST_BOX_HOT_PROPERTY).is_ok_and(|handle| !handle.is_invalid()) {
        let _ = InvalidateRect(hwnd, None, false);
    }
}

unsafe fn paint_list_box_rows(hwnd: HWND, palette: Palette) {
    const LB_GETCOUNT: u32 = 0x018b;
    const LB_GETCURSEL: u32 = 0x0188;
    const LB_GETITEMRECT: u32 = 0x0198;
    const LB_GETTEXT: u32 = 0x0189;
    const LB_GETTEXTLEN: u32 = 0x018a;
    const LB_GETTOPINDEX: u32 = 0x018e;
    let count = SendMessageW(hwnd, LB_GETCOUNT, WPARAM(0), LPARAM(0)).0;
    if count <= 0 {
        return;
    }
    let selected = SendMessageW(hwnd, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    let hot = GetPropW(hwnd, LIST_BOX_HOT_PROPERTY);
    let hot = (!hot.is_invalid()).then_some(hot.0 as usize - 1);
    let top = SendMessageW(hwnd, LB_GETTOPINDEX, WPARAM(0), LPARAM(0))
        .0
        .max(0) as usize;
    let dc = windows::Win32::Graphics::Gdi::GetDC(hwnd);
    if dc.is_invalid() {
        return;
    }
    let font = SendMessageW(hwnd, WM_GETFONT, WPARAM(0), LPARAM(0));
    let old_font = (font.0 != 0)
        .then(|| SelectObject(dc, windows::Win32::Graphics::Gdi::HGDIOBJ(font.0 as *mut _)));
    let _ = SetBkMode(dc, TRANSPARENT);
    let inset = scale(7, GetDpiForWindow(hwnd).max(96));
    for index in top..count as usize {
        let mut row = RECT::default();
        if SendMessageW(
            hwnd,
            LB_GETITEMRECT,
            WPARAM(index),
            LPARAM((&mut row as *mut RECT) as isize),
        )
        .0 < 0
        {
            break;
        }
        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);
        if row.top >= client.bottom {
            break;
        }
        let is_selected = selected >= 0 && selected as usize == index;
        let is_hot = hot == Some(index);
        let (text_color, background) = if is_selected || is_hot {
            navigation_selection_colors(palette, is_hot)
        } else {
            (palette.text, palette.edit)
        };
        fill(dc, &row, background);
        let length = SendMessageW(hwnd, LB_GETTEXTLEN, WPARAM(index), LPARAM(0)).0;
        if length < 0 {
            continue;
        }
        let mut text = vec![0u16; length as usize + 1];
        let _ = SendMessageW(
            hwnd,
            LB_GETTEXT,
            WPARAM(index),
            LPARAM(text.as_mut_ptr() as isize),
        );
        let _ = SetTextColor(dc, text_color);
        row.left += inset;
        row.right -= inset;
        let _ = DrawTextW(
            dc,
            &mut text,
            &mut row,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }
    if let Some(old_font) = old_font {
        let _ = SelectObject(dc, old_font);
    }
    let _ = windows::Win32::Graphics::Gdi::ReleaseDC(hwnd, dc);
}

unsafe extern "system" fn list_view_parent_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    subclass_id: usize,
    reference_data: usize,
) -> LRESULT {
    match message {
        WM_NOTIFY if lparam.0 != 0 => {
            let draw = &mut *(lparam.0 as *mut NMLVCUSTOMDRAW);
            let dark_flag = 1usize << (usize::BITS - 1);
            let list = HWND((reference_data & !dark_flag) as *mut _);
            if draw.nmcd.hdr.hwndFrom == list && draw.nmcd.hdr.code == NM_CUSTOMDRAW {
                let palette = palette_from_reference(usize::from(reference_data & dark_flag != 0));
                if draw.nmcd.dwDrawStage == CDDS_PREPAINT {
                    return LRESULT(CDRF_NOTIFYITEMDRAW as isize);
                }
                if draw.nmcd.dwDrawStage == CDDS_ITEMPREPAINT {
                    // Always remove the native selected bit from this transient paint snapshot.
                    // Clearing it only for the currently selected row allows a stale focused row
                    // to be overpainted with the system-blue selection after the real selection
                    // has already moved elsewhere.
                    draw.nmcd.uItemState.0 = list_view_custom_draw_state(draw.nmcd.uItemState.0);
                    // `uItemState` is a custom-draw state snapshot, not the authoritative
                    // ListView selection state. Depending on comctl32 version and focus changes it
                    // can retain CDIS_SELECTED for rows that are no longer selected. Query the
                    // row itself so only the actual LVIS_SELECTED item receives the highlight.
                    const LVM_GETITEMSTATE: u32 = 0x102c;
                    const LVIS_SELECTED: isize = 0x0002;
                    let item_state = SendMessageW(
                        list,
                        LVM_GETITEMSTATE,
                        WPARAM(draw.nmcd.dwItemSpec),
                        LPARAM(LVIS_SELECTED),
                    )
                    .0;
                    let selected = item_state & LVIS_SELECTED != 0;
                    if selected && paint_selected_list_view_row(list, draw, palette) {
                        // Windows 11's v6 ItemsView theme paints COLOR_HIGHLIGHT over clrTextBk
                        // after NM_CUSTOMDRAW.  Skip only that one selected row after reproducing
                        // its report-mode text layout; every unselected row remains native.
                        return LRESULT((CDRF_SKIPDEFAULT | CDRF_SKIPPOSTPAINT) as isize);
                    }
                    return LRESULT(CDRF_DODEFAULT as isize);
                }
                return LRESULT(CDRF_DODEFAULT as isize);
            }
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, Some(list_view_parent_subclass), subclass_id);
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, message, wparam, lparam),
    }
}

unsafe fn paint_selected_list_view_row(
    list: HWND,
    draw: &mut NMLVCUSTOMDRAW,
    palette: Palette,
) -> bool {
    const LVM_GETHEADER: u32 = 0x101f;
    const LVM_GETITEMRECT: u32 = 0x100e;
    const LVM_GETITEMSTATE: u32 = 0x102c;
    const LVM_GETSUBITEMRECT: u32 = 0x1038;
    const LVM_GETITEMTEXTW: u32 = 0x1073;
    const HDM_GETITEMCOUNT: u32 = 0x1200;
    const LVIR_BOUNDS: i32 = 0;
    const LVIS_STATEIMAGEMASK: isize = 0xf000;

    let item_index = draw.nmcd.dwItemSpec;
    let mut row = RECT {
        left: LVIR_BOUNDS,
        ..Default::default()
    };
    if SendMessageW(
        list,
        LVM_GETITEMRECT,
        WPARAM(item_index),
        LPARAM((&mut row as *mut RECT) as isize),
    )
    .0 == 0
    {
        return false;
    }

    let mut client = RECT::default();
    if GetClientRect(list, &mut client).is_err() {
        return false;
    }
    row.left = row.left.max(client.left);
    row.top = row.top.max(client.top);
    row.right = row.right.min(client.right);
    row.bottom = row.bottom.min(client.bottom);
    if row.right <= row.left || row.bottom <= row.top {
        return false;
    }

    let (text_color, selection_fill) = list_view_row_colors(palette, true);
    fill(draw.nmcd.hdc, &row, selection_fill);

    let font = SendMessageW(list, WM_GETFONT, WPARAM(0), LPARAM(0));
    let old_font = (font.0 != 0).then(|| {
        SelectObject(
            draw.nmcd.hdc,
            windows::Win32::Graphics::Gdi::HGDIOBJ(font.0 as *mut _),
        )
    });
    let _ = SetBkMode(draw.nmcd.hdc, TRANSPARENT);
    let _ = SetTextColor(draw.nmcd.hdc, text_color);

    let header = HWND(SendMessageW(list, LVM_GETHEADER, WPARAM(0), LPARAM(0)).0 as *mut _);
    let column_count = if header.0.is_null() {
        1
    } else {
        SendMessageW(header, HDM_GETITEMCOUNT, WPARAM(0), LPARAM(0))
            .0
            .max(1) as i32
    };
    let dpi = GetDpiForWindow(list).max(96);
    let inset = scale(7, dpi);
    let state_image = SendMessageW(
        list,
        LVM_GETITEMSTATE,
        WPARAM(item_index),
        LPARAM(LVIS_STATEIMAGEMASK),
    )
    .0 as u32;

    for subitem in 0..column_count {
        let mut text_rect = RECT {
            left: LVIR_BOUNDS,
            top: subitem,
            ..Default::default()
        };
        if SendMessageW(
            list,
            LVM_GETSUBITEMRECT,
            WPARAM(item_index),
            LPARAM((&mut text_rect as *mut RECT) as isize),
        )
        .0 == 0
        {
            continue;
        }
        text_rect.left = text_rect.left.max(client.left);
        text_rect.right = text_rect.right.min(client.right);
        if text_rect.right <= text_rect.left {
            continue;
        }

        let mut text = vec![0u16; 1024];
        let mut item = LVITEMW {
            mask: LVIF_TEXT,
            iSubItem: subitem,
            pszText: PWSTR(text.as_mut_ptr()),
            cchTextMax: text.len() as i32,
            ..Default::default()
        };
        let copied = SendMessageW(
            list,
            LVM_GETITEMTEXTW,
            WPARAM(item_index),
            LPARAM((&mut item as *mut LVITEMW) as isize),
        )
        .0
        .max(0) as usize;
        text.truncate(copied.min(text.len()));

        text_rect.left += inset;
        if subitem == 0 && state_image & LVIS_STATEIMAGEMASK as u32 != 0 {
            // The deterministic checkbox painter replaces the native state image after WM_PAINT.
            // Reserve the same leading slot so selected-row text never overlaps its glyph.
            text_rect.left += scale(24, dpi);
        }
        text_rect.right -= inset.min((text_rect.right - text_rect.left).max(0));
        let _ = DrawTextW(
            draw.nmcd.hdc,
            &mut text,
            &mut text_rect,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }

    if let Some(old_font) = old_font {
        let _ = SelectObject(draw.nmcd.hdc, old_font);
    }
    true
}

fn list_view_row_colors(palette: Palette, selected: bool) -> (COLORREF, COLORREF) {
    if selected {
        // A selected report row must stay identical to the resting selected navigation button.
        // Pointer hover must not silently switch it to the brighter hot-button colour.
        navigation_selection_colors(palette, false)
    } else {
        (palette.text, palette.edit)
    }
}

/// Reuses the exact normal/hot palette of the selected left navigation entry. Keeping this as the
/// single source of truth prevents report rows and standalone lists drifting back to system blue.
fn navigation_selection_colors(palette: Palette, hot: bool) -> (COLORREF, COLORREF) {
    let visual = button_visual(
        palette,
        ButtonRole::Navigation { selected: true },
        ControlState {
            hot,
            ..ControlState::default()
        },
    );
    (visual.text, visual.fill)
}

/// Clears native selection, hot and focus paint bits from the transient custom-draw snapshot. The
/// authoritative ListView item state is queried separately and remains unchanged; suppressing the
/// snapshot bits prevents the system light theme from painting a white/focus overlay after the
/// application supplied the selected navigation colour.
const fn list_view_custom_draw_state(snapshot: u32) -> u32 {
    const CDIS_SELECTED: u32 = 0x0001;
    const CDIS_FOCUS: u32 = 0x0010;
    const CDIS_HOT: u32 = 0x0040;
    snapshot & !(CDIS_SELECTED | CDIS_FOCUS | CDIS_HOT)
}

unsafe fn paint_list_view_checkboxes(hwnd: HWND, palette: Palette) {
    const LVIS_STATEIMAGEMASK: isize = 0xf000;
    let top = SendMessageW(hwnd, 0x1027, WPARAM(0), LPARAM(0)).0.max(0) as i32; // TOPINDEX
    let visible = SendMessageW(hwnd, 0x1028, WPARAM(0), LPARAM(0)).0.max(0) as i32; // PERPAGE
    let count = SendMessageW(hwnd, 0x1004, WPARAM(0), LPARAM(0)).0.max(0) as i32; // ITEMCOUNT
    if count == 0 {
        return;
    }
    let dc = windows::Win32::Graphics::Gdi::GetDC(hwnd);
    if dc.is_invalid() {
        return;
    }
    let dpi = GetDpiForWindow(hwnd).max(96);
    let size = scale(14, dpi).max(10);
    for index in top..(top + visible + 1).min(count) {
        let state = SendMessageW(
            hwnd,
            0x102C, // LVM_GETITEMSTATE
            WPARAM(index as usize),
            LPARAM(LVIS_STATEIMAGEMASK),
        )
        .0 as u32;
        let state_image = (state >> 12) & 0xf;
        if state_image == 0 {
            continue;
        }
        let mut row = RECT {
            left: 0, // LVIR_BOUNDS
            ..Default::default()
        };
        if SendMessageW(
            hwnd,
            0x100E, // LVM_GETITEMRECT
            WPARAM(index as usize),
            LPARAM((&mut row as *mut RECT) as isize),
        )
        .0 == 0
        {
            continue;
        }
        // The state image precedes LVIR_ICON. Painting inside LVIR_ICON produced the visible
        // double-checkbox regression (native white state image plus our dark box). Clear and
        // replace the actual leading state-image slot instead.
        let selected = SendMessageW(
            hwnd,
            0x102C, // LVM_GETITEMSTATE
            WPARAM(index as usize),
            LPARAM(0x0002), // LVIS_SELECTED
        )
        .0 != 0;
        let slot = RECT {
            left: row.left,
            top: row.top,
            right: row.left + scale(24, dpi),
            bottom: row.bottom,
        };
        fill(
            dc,
            &slot,
            if selected {
                palette.accent_fill
            } else {
                palette.edit
            },
        );
        let top = row.top + ((row.bottom - row.top - size) / 2).max(0);
        let left = row.left + scale(4, dpi);
        let box_rect = RECT {
            left,
            top,
            right: left + size,
            bottom: top + size,
        };
        let checked = state_image == 2;
        fill(
            dc,
            &box_rect,
            if checked {
                palette.progress
            } else {
                palette.edit
            },
        );
        stroke(
            dc,
            box_rect,
            if checked {
                palette.progress
            } else {
                palette.text_secondary
            },
        );
        if checked {
            let pen = CreatePen(PEN_STYLE(0), scale(2, dpi).max(1), COLORREF(0x00ff_ffff));
            let old_pen = SelectObject(dc, pen);
            let _ = MoveToEx(dc, left + size / 5, top + size / 2, None);
            let _ = LineTo(dc, left + size * 2 / 5, top + size * 3 / 4);
            let _ = LineTo(dc, left + size * 4 / 5, top + size / 4);
            let _ = SelectObject(dc, old_pen);
            let _ = DeleteObject(pen);
        }
    }
    let _ = windows::Win32::Graphics::Gdi::ReleaseDC(hwnd, dc);
}

unsafe extern "system" fn progress_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    reference_data: usize,
) -> LRESULT {
    match message {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            paint_progress(hwnd, palette_from_reference(reference_data));
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, Some(progress_subclass), PROGRESS_SUBCLASS_ID);
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
        _ => {
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            if (0x0401..=0x0410).contains(&message) {
                let _ = InvalidateRect(hwnd, None, false);
            }
            result
        }
    }
}

unsafe fn paint_progress(hwnd: HWND, palette: Palette) {
    let mut paint = PAINTSTRUCT::default();
    let dc = BeginPaint(hwnd, &mut paint);
    let mut rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut rect);
    let position = SendMessageW(hwnd, 0x0408, WPARAM(0), LPARAM(0)).0.max(0) as u64;
    let maximum = SendMessageW(hwnd, 0x0407, WPARAM(0), LPARAM(0)).0.max(1) as u64;
    draw_progress(dc, rect, position, maximum, ProgressRole::Normal, palette);
    let _ = EndPaint(hwnd, &paint);
}

unsafe extern "system" fn trackbar_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    reference_data: usize,
) -> LRESULT {
    match message {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            paint_trackbar(hwnd, palette_from_reference(reference_data));
            LRESULT(0)
        }
        WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_CAPTURECHANGED | WM_KEYDOWN
        | WM_KEYUP | WM_SETFOCUS | WM_KILLFOCUS | WM_ENABLE => {
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            let _ = InvalidateRect(hwnd, None, false);
            result
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, Some(trackbar_subclass), TRACKBAR_SUBCLASS_ID);
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
        _ => {
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            // Only setters invalidate. paint_trackbar reads TBM_GETPOS,
            // TBM_GETRANGEMIN and TBM_GETRANGEMAX; treating those queries as mutations causes
            // WM_PAINT -> TBM_GET* -> synchronous WM_PAINT recursion and leaves every child after
            // the slider with its initial white USER32 surface.
            if matches!(message, 0x0405..=0x0408) {
                let _ = InvalidateRect(hwnd, None, false);
            }
            result
        }
    }
}

unsafe fn paint_trackbar(hwnd: HWND, palette: Palette) {
    let mut paint = PAINTSTRUCT::default();
    let dc = BeginPaint(hwnd, &mut paint);
    let mut rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut rect);
    let width = (rect.right - rect.left).max(0);
    let height = (rect.bottom - rect.top).max(0);
    if width == 0 || height == 0 {
        let _ = EndPaint(hwnd, &paint);
        return;
    }
    let memory_dc = CreateCompatibleDC(dc);
    let bitmap = CreateCompatibleBitmap(dc, width, height);
    if memory_dc.is_invalid() || bitmap.is_invalid() {
        if !memory_dc.is_invalid() {
            let _ = DeleteDC(memory_dc);
        }
        if !bitmap.is_invalid() {
            let _ = DeleteObject(bitmap);
        }
        let _ = EndPaint(hwnd, &paint);
        return;
    }
    let old_bitmap = SelectObject(memory_dc, bitmap);
    let local = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    fill(memory_dc, &local, palette.window);
    let dpi = GetDpiForWindow(hwnd).max(96);
    let thumb_width = scale(14, dpi).max(10);
    let thumb_height = scale(22, dpi).min(height);
    let left = thumb_width / 2;
    let right = (width - thumb_width / 2).max(left);
    let center = height / 2;
    let minimum = SendMessageW(hwnd, 0x0401, WPARAM(0), LPARAM(0)).0 as i64;
    let maximum = SendMessageW(hwnd, 0x0402, WPARAM(0), LPARAM(0)).0 as i64;
    let position = SendMessageW(hwnd, 0x0400, WPARAM(0), LPARAM(0)).0 as i64;
    let span = (maximum - minimum).max(1);
    let x = left + (((right - left) as i64 * (position - minimum).clamp(0, span)) / span) as i32;
    let channel = RECT {
        left,
        top: center - scale(3, dpi),
        right,
        bottom: center + scale(3, dpi),
    };
    let channel_radius = ((channel.bottom - channel.top) / 2).max(2);
    fill_round_rect_antialiased(
        memory_dc,
        channel,
        channel_radius,
        palette.edit,
        palette.border,
        palette.window,
    );
    let selected = RECT {
        right: x,
        ..channel
    };
    if selected.right > selected.left {
        fill_round_rect_antialiased(
            memory_dc,
            selected,
            channel_radius,
            palette.progress,
            palette.progress,
            palette.edit,
        );
    }
    let thumb = RECT {
        left: x - thumb_width / 2,
        top: center - thumb_height / 2,
        right: x + (thumb_width + 1) / 2,
        bottom: center + (thumb_height + 1) / 2,
    };
    let enabled = IsWindowEnabled(hwnd).as_bool();
    fill_round_rect_antialiased(
        memory_dc,
        thumb,
        (thumb_width / 2).max(3),
        if enabled {
            palette.button
        } else {
            palette.window
        },
        if enabled {
            palette.text_secondary
        } else {
            palette.text_disabled
        },
        palette.window,
    );
    let _ = BitBlt(dc, 0, 0, width, height, memory_dc, 0, 0, SRCCOPY);
    let _ = SelectObject(memory_dc, old_bitmap);
    let _ = DeleteObject(bitmap);
    let _ = DeleteDC(memory_dc);
    let _ = EndPaint(hwnd, &paint);
}

fn scale(value: i32, dpi: u32) -> i32 {
    ((value as i64 * dpi.max(1) as i64 + 48) / 96) as i32
}

unsafe fn fill(dc: windows::Win32::Graphics::Gdi::HDC, rect: &RECT, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    let _ = FillRect(dc, rect, brush);
    let _ = DeleteObject(brush);
}

unsafe fn stroke(dc: windows::Win32::Graphics::Gdi::HDC, rect: RECT, color: COLORREF) {
    let pen = CreatePen(PEN_STYLE(0), 1, color);
    let hollow =
        windows::Win32::Graphics::Gdi::GetStockObject(windows::Win32::Graphics::Gdi::NULL_BRUSH);
    let old_pen = SelectObject(dc, pen);
    let old_brush = SelectObject(dc, hollow);
    let _ =
        windows::Win32::Graphics::Gdi::Rectangle(dc, rect.left, rect.top, rect.right, rect.bottom);
    let _ = SelectObject(dc, old_brush);
    let _ = SelectObject(dc, old_pen);
    let _ = DeleteObject(pen);
}

unsafe fn round_rect(
    dc: windows::Win32::Graphics::Gdi::HDC,
    rect: RECT,
    radius: i32,
    background: COLORREF,
    border: COLORREF,
) {
    let brush = CreateSolidBrush(background);
    let pen = CreatePen(PEN_STYLE(0), 1, border);
    let old_brush = SelectObject(dc, brush);
    let old_pen = SelectObject(dc, pen);
    let diameter = radius.saturating_mul(2);
    let _ = RoundRect(
        dc,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        diameter,
        diameter,
    );
    let _ = SelectObject(dc, old_pen);
    let _ = SelectObject(dc, old_brush);
    let _ = DeleteObject(pen);
    let _ = DeleteObject(brush);
}

pub struct Brushes {
    pub window: HBRUSH,
    pub nav: HBRUSH,
    pub edit: HBRUSH,
}

impl Brushes {
    pub fn new(palette: Palette) -> Self {
        unsafe {
            Self {
                window: CreateSolidBrush(palette.window),
                nav: CreateSolidBrush(palette.nav),
                edit: CreateSolidBrush(palette.edit),
            }
        }
    }
}

impl Drop for Brushes {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.window);
            let _ = DeleteObject(self.nav);
            let _ = DeleteObject(self.edit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inno_windows11_reference_colors_are_stable() {
        assert_eq!(Palette::LIGHT.window, rgb(249, 249, 249));
        assert_eq!(Palette::LIGHT.edit, rgb(255, 255, 255));
        assert_eq!(Palette::LIGHT.border, rgb(230, 230, 230));
        assert_eq!(Palette::LIGHT.separator, rgb(222, 222, 222));
        assert_eq!(Palette::LIGHT.accent_fill, rgb(0, 95, 184));
        assert_eq!(Palette::DARK.window, rgb(43, 43, 43));
        assert_eq!(Palette::DARK.edit, rgb(31, 31, 31));
        assert_eq!(Palette::DARK.separator, rgb(81, 81, 81));
        assert_eq!(Palette::DARK.accent_border, rgb(66, 149, 192));
    }

    #[test]
    fn native_theme_classes_cover_headers_scrollbars_and_fields() {
        assert_eq!(
            native_theme_class(NativeControlKind::Header, true),
            NativeThemeClass::DarkItemsView
        );
        assert_eq!(
            native_theme_class(NativeControlKind::ScrollableField, true),
            NativeThemeClass::DarkExplorer
        );
        assert_eq!(
            native_theme_class(NativeControlKind::List, true),
            NativeThemeClass::DarkExplorer
        );
        assert_eq!(
            native_theme_class(NativeControlKind::Field, true),
            NativeThemeClass::DarkCfd
        );
        assert_eq!(
            native_theme_class(NativeControlKind::Field, false),
            NativeThemeClass::Cfd
        );
    }

    #[test]
    fn field_styles_keep_property_page_edit_and_remove_competing_rounded_edges() {
        assert!(is_edit_class("Edit"));
        assert!(is_edit_class("EDIT"));
        assert!(!is_edit_class("ComboBox"));
        assert!(is_combo_class("ComboBox"));
        let (style, ex_style) = borderless_style_bits(
            0x1000 | WS_BORDER.0 as isize,
            WS_EX_CLIENTEDGE.0 as isize | 0x2000,
        );
        assert_eq!(style & WS_BORDER.0 as isize, 0);
        assert_eq!(ex_style & WS_EX_CLIENTEDGE.0 as isize, 0);
        assert_ne!(ex_style & 0x2000, 0);

        let (edit_style, edit_ex_style) =
            property_page_edit_style_bits(0x1000 | WS_BORDER.0 as isize, 0x0004);
        assert_eq!(edit_style & WS_BORDER.0 as isize, 0);
        assert_ne!(edit_ex_style & WS_EX_CLIENTEDGE.0 as isize, 0);
        assert_ne!(edit_ex_style & 0x0004, 0);

        let (list_style, list_ex_style) =
            single_border_style_bits(0x1000, WS_EX_CLIENTEDGE.0 as isize | 0x2000);
        assert_ne!(list_style & WS_BORDER.0 as isize, 0);
        assert_eq!(list_ex_style & WS_EX_CLIENTEDGE.0 as isize, 0);
    }

    #[test]
    fn list_view_selection_matches_navigation_and_does_not_shift_on_hover() {
        let dark_nav = button_visual(
            Palette::DARK,
            ButtonRole::Navigation { selected: true },
            ControlState::default(),
        );
        let light_nav = button_visual(
            Palette::LIGHT,
            ButtonRole::Navigation { selected: true },
            ControlState::default(),
        );
        assert_eq!(
            list_view_row_colors(Palette::DARK, false),
            (Palette::DARK.text, Palette::DARK.edit)
        );
        assert_eq!(
            list_view_row_colors(Palette::DARK, true),
            (dark_nav.text, dark_nav.fill)
        );
        assert_eq!(
            list_view_row_colors(Palette::LIGHT, true),
            (light_nav.text, light_nav.fill)
        );
        assert_eq!(
            list_view_row_colors(Palette::DARK, false),
            (Palette::DARK.text, Palette::DARK.edit)
        );
        assert_eq!(
            list_view_row_colors(Palette::LIGHT, false),
            (Palette::LIGHT.text, Palette::LIGHT.edit)
        );
        assert_eq!(
            list_view_row_colors(Palette::DARK, true),
            (dark_nav.text, dark_nav.fill)
        );
        assert_ne!(light_nav.fill, Palette::LIGHT.edit);
        assert_ne!(light_nav.fill, Palette::LIGHT.window);
        assert_ne!(Palette::DARK.accent_fill, Palette::DARK.progress);
    }

    #[test]
    fn empty_list_view_owns_its_body_paint() {
        assert!(list_view_needs_empty_body_paint(0));
        assert!(!list_view_needs_empty_body_paint(1));
        assert!(!list_view_needs_empty_body_paint(32));
    }

    #[test]
    fn stale_list_view_snapshot_never_overrides_real_selection_colors() {
        const CDIS_SELECTED: u32 = 0x0001;
        const CDIS_FOCUS: u32 = 0x0010;
        const CDIS_HOT: u32 = 0x0040;
        let stale_snapshot = CDIS_SELECTED | CDIS_FOCUS | CDIS_HOT;

        assert_eq!(list_view_custom_draw_state(stale_snapshot), 0);
        assert_eq!(
            list_view_row_colors(Palette::DARK, false),
            (Palette::DARK.text, Palette::DARK.edit)
        );
        assert_eq!(
            list_view_row_colors(Palette::DARK, true),
            (Palette::DARK.text, Palette::DARK.accent_fill)
        );
    }

    #[test]
    fn native_theme_still_supplies_control_content_beneath_deterministic_frames() {
        assert_eq!(
            native_theme_class(NativeControlKind::Field, true),
            NativeThemeClass::DarkCfd
        );
        assert_eq!(
            native_theme_class(NativeControlKind::ListView, true),
            NativeThemeClass::DarkExplorer
        );
        assert_eq!(
            native_theme_class(NativeControlKind::Field, false),
            NativeThemeClass::Cfd
        );
    }

    #[test]
    fn dark_radio_fallback_is_limited_to_real_auto_radio_buttons() {
        assert!(button_style_is_auto_radio(0x0009));
        assert!(button_style_is_auto_radio(0x5001_0009));
        assert!(!button_style_is_auto_radio(0x0003)); // auto checkbox
        assert!(!is_auto_radio_button("Static", 0x0009));
        assert!(is_auto_radio_button("BUTTON", 0x0009));
    }
}
