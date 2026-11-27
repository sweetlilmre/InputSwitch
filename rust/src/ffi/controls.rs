//! Standard and common controls: buttons, edit boxes, labels, list boxes, combo boxes, list
//! views, owner-draw and custom-draw, subclassing, and TaskDialog.

use super::*;

#[repr(C)]
pub struct NMHDR {
    pub hwndFrom: HWND,
    pub idFrom: usize,
    pub code: u32,
}

#[repr(C)]
pub struct NMLISTVIEW {
    pub hdr: NMHDR,
    pub iItem: i32,
    pub iSubItem: i32,
    pub uNewState: u32,
    pub uOldState: u32,
    pub uChanged: u32,
    pub ptAction: POINT,
    pub lParam: LPARAM,
}

#[repr(C)]
pub struct LVCOLUMNW {
    pub mask: u32,
    pub fmt: i32,
    pub cx: i32,
    pub pszText: *mut u16,
    pub cchTextMax: i32,
    pub iSubItem: i32,
    pub iImage: i32,
    pub iOrder: i32,
    pub cxMin: i32,
    pub cxDefault: i32,
    pub cxIdeal: i32,
}

#[repr(C)]
pub struct LVITEMW {
    pub mask: u32,
    pub iItem: i32,
    pub iSubItem: i32,
    pub state: u32,
    pub stateMask: u32,
    pub pszText: *mut u16,
    pub cchTextMax: i32,
    pub iImage: i32,
    pub lParam: LPARAM,
    pub iIndent: i32,
    pub iGroupId: i32,
    pub cColumns: u32,
    pub puColumns: *mut u32,
    pub piColFmt: *mut i32,
    pub iGroup: i32,
}

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

#[repr(C)]
pub struct NMCUSTOMDRAW {
    pub hdr: NMHDR,
    pub dwDrawStage: u32,
    pub hdc: HDC,
    pub rc: RECT,
    pub dwItemSpec: usize,
    pub uItemState: u32,
    pub lItemlParam: LPARAM,
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
pub const CBS_DROPDOWNLIST: u32 = 0x0003;
pub const CB_ADDSTRING: u32 = 0x0143;
pub const CB_GETCURSEL: u32 = 0x0147;
pub const CB_RESETCONTENT: u32 = 0x014B;
pub const CB_SETCURSEL: u32 = 0x014E;
pub const CBN_SELCHANGE: u16 = 1;
pub const IDOK: i32 = 1;
pub const IDCANCEL: i32 = 2;
pub const LVS_REPORT: u32 = 0x0001;
pub const LVS_SINGLESEL: u32 = 0x0004;
pub const LVS_SHOWSELALWAYS: u32 = 0x0008;
pub const LVS_NOCOLUMNHEADER: u32 = 0x4000;
pub const LVS_NOSORTHEADER: u32 = 0x8000;
pub const LVS_EX_CHECKBOXES: u32 = 0x0004;
pub const LVS_EX_FULLROWSELECT: u32 = 0x0020;
pub const LVS_EX_DOUBLEBUFFER: u32 = 0x0001_0000;
pub const LVM_GETITEMCOUNT: u32 = 0x1004;
pub const LVM_DELETEALLITEMS: u32 = 0x1009;
pub const LVM_GETNEXTITEM: u32 = 0x100C;
pub const LVM_ENSUREVISIBLE: u32 = 0x1013;
pub const LVM_SETCOLUMNWIDTH: u32 = 0x101E;
pub const LVM_SETITEMSTATE: u32 = 0x102B;
pub const LVM_GETITEMSTATE: u32 = 0x102C;
pub const LVM_SETEXTENDEDLISTVIEWSTYLE: u32 = 0x1036;
pub const LVM_GETITEMW: u32 = 0x104B;
pub const LVM_INSERTITEMW: u32 = 0x104D;
pub const LVM_INSERTCOLUMNW: u32 = 0x1061;
pub const LVM_SETITEMTEXTW: u32 = 0x1074;
pub const LVIF_TEXT: u32 = 0x1;
pub const LVIF_STATE: u32 = 0x8;
pub const LVIF_PARAM: u32 = 0x4;
pub const LVCF_TEXT: u32 = 0x4;
pub const LVCF_WIDTH: u32 = 0x2;
pub const LVIS_FOCUSED: u32 = 0x1;
pub const LVIS_SELECTED: u32 = 0x2;
pub const LVIS_STATEIMAGEMASK: u32 = 0xF000;
pub const LVNI_SELECTED: isize = 0x2;
pub const LVN_ITEMCHANGED: u32 = (-101i32) as u32;
pub const ICC_LISTVIEW_CLASSES: u32 = 0x0001;
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
pub const LVM_SETBKCOLOR: u32 = 0x1001;
pub const LVM_GETHEADER: u32 = 0x101F;
pub const LVM_SETTEXTCOLOR: u32 = 0x1024;
pub const LVM_SETTEXTBKCOLOR: u32 = 0x1026;
pub const LVM_SETITEMW: u32 = 0x104C;
pub const LVSCW_AUTOSIZE_USEHEADER: isize = -2;
pub const EN_SETFOCUS: u16 = 0x0100;
pub const EN_KILLFOCUS: u16 = 0x0200;
pub const NM_CUSTOMDRAW: u32 = (-12i32) as u32;
pub const CDDS_PREPAINT: u32 = 0x0000_0001;
pub const CDDS_ITEMPREPAINT: u32 = 0x0001_0001;
pub const CDRF_DODEFAULT: LRESULT = 0;
pub const CDRF_NOTIFYITEMDRAW: LRESULT = 0x20;

#[link(name = "comctl32")]
extern "system" {
    pub fn InitCommonControlsEx(icc: *const INITCOMMONCONTROLSEX) -> BOOL;
    pub fn TaskDialog(owner: HWND, inst: HINSTANCE, title: *const u16, main: *const u16, content: *const u16,
        buttons: u32, icon: *const u16, pressed: *mut i32) -> i32;
    pub fn SetWindowSubclass(h: HWND, proc: SUBCLASSPROC, id: usize, data: usize) -> BOOL;
    pub fn RemoveWindowSubclass(h: HWND, proc: SUBCLASSPROC, id: usize) -> BOOL;
    pub fn DefSubclassProc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT;
}
