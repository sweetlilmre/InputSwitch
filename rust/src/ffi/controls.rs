//! Standard and common controls: buttons, edit boxes, labels, list boxes,
//! owner-draw, subclassing, and TaskDialog.

use super::*;

#[repr(C)]
pub struct INITCOMMONCONTROLSEX {
    pub dwSize: u32,
    pub dwICC: u32,
}

pub type SUBCLASSPROC = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM, usize, usize) -> LRESULT;

#[repr(C)]
pub struct DRAWITEMSTRUCT {
    pub CtlType: u32,
    pub CtlID: u32,
    pub itemID: u32,
    pub itemAction: u32,
    pub itemState: u32,
    pub hwndItem: HWND,
    pub hDC: HDC,
    pub rcItem: RECT,
    pub itemData: usize,
}

#[repr(C)]
pub struct MEASUREITEMSTRUCT {
    pub CtlType: u32,
    pub CtlID: u32,
    pub itemID: u32,
    pub itemWidth: u32,
    pub itemHeight: u32,
    pub itemData: usize,
}

pub const BN_CLICKED: u16 = 0;
pub const ES_AUTOHSCROLL: u32 = 0x0080;
pub const EN_CHANGE: u16 = 0x0300;
pub const SS_ENDELLIPSIS: u32 = 0x4000;
pub const SS_NOPREFIX: u32 = 0x0080;
pub const SS_RIGHT: u32 = 0x0002;
pub const LBS_NOTIFY: u32 = 0x0001;
pub const LBS_NOINTEGRALHEIGHT: u32 = 0x0100;
pub const LB_ADDSTRING: u32 = 0x0180;
pub const LB_INSERTSTRING: u32 = 0x0181;
pub const LB_DELETESTRING: u32 = 0x0182;
pub const LB_RESETCONTENT: u32 = 0x0184;
pub const LB_SETCURSEL: u32 = 0x0186;
pub const LB_GETCURSEL: u32 = 0x0188;
pub const LBN_SELCHANGE: u16 = 1;
pub const IDOK: i32 = 1;
pub const IDCANCEL: i32 = 2;
pub const ICC_STANDARD_CLASSES: u32 = 0x4000;
pub const ES_READONLY: u32 = 0x0800;
pub const TDCBF_OK_BUTTON: u32 = 0x1;
pub const TDCBF_CANCEL_BUTTON: u32 = 0x8;
pub const TD_WARNING_ICON: *const u16 = 0xFFFF as *const u16;
pub const TD_INFORMATION_ICON: *const u16 = 0xFFFD as *const u16;
pub const BS_OWNERDRAW: u32 = 0x000B;
pub const LBS_OWNERDRAWFIXED: u32 = 0x0010;
pub const LBS_HASSTRINGS: u32 = 0x0040;
pub const LB_GETTEXT: u32 = 0x0189;
pub const LB_GETTEXTLEN: u32 = 0x018A;
pub const LB_SETITEMHEIGHT: u32 = 0x01A0;
pub const EM_SETMARGINS: u32 = 0x00D3;
pub const EM_SETSEL: u32 = 0x00B1;
pub const EC_LEFTMARGIN: usize = 0x1;
pub const EC_RIGHTMARGIN: usize = 0x2;
pub const ODT_LISTBOX: u32 = 2;
pub const ODT_BUTTON: u32 = 4;
pub const ODS_SELECTED: u32 = 0x0001;
pub const ODS_DISABLED: u32 = 0x0004;
pub const ODS_FOCUS: u32 = 0x0010;
pub const ODS_NOFOCUSRECT: u32 = 0x0200;
pub const EN_SETFOCUS: u16 = 0x0100;
pub const EN_KILLFOCUS: u16 = 0x0200;

#[link(name = "comctl32")]
extern "system" {
    pub fn InitCommonControlsEx(icc: *const INITCOMMONCONTROLSEX) -> BOOL;
    pub fn TaskDialog(owner: HWND, inst: HINSTANCE, title: *const u16, main: *const u16, content: *const u16,
        buttons: u32, icon: *const u16, pressed: *mut i32) -> i32;
    pub fn SetWindowSubclass(h: HWND, proc: SUBCLASSPROC, id: usize, data: usize) -> BOOL;
    pub fn RemoveWindowSubclass(h: HWND, proc: SUBCLASSPROC, id: usize) -> BOOL;
    pub fn DefSubclassProc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT;
}
