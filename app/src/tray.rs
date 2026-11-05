use std::cell::{Cell, RefCell};
use std::ptr;
use slint::ComponentHandle;
use raw_window_handle::HasWindowHandle;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::{LibraryLoader::GetModuleHandleW, Registry::{
        RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD,
    }},
    UI::{
        HiDpi::GetDpiForWindow,
        Shell::{ExtractIconW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW},
        WindowsAndMessaging::{
            AppendMenuW, CallWindowProcW, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos,            GetSystemMetrics, GetWindowLongPtrW, HICON, IsIconic, LoadIconW, MF_SEPARATOR, MF_STRING,
            PostMessageW, SetForegroundWindow, SetWindowLongPtrW, ShowWindow, TrackPopupMenu,
            GWLP_WNDPROC, IDI_APPLICATION, SM_CXSMICON, SW_HIDE, SW_RESTORE, SW_SHOW, TPM_RETURNCMD, TPM_RIGHTBUTTON,
            WM_APP, WM_CLOSE, WM_COMMAND, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP,
        },
    },
};

const MESSAGE: u32 = WM_APP + 1;
const UID: u32 = 0x1f1f0c05;
const OPEN: usize = 1;
const QUIT: usize = 2;
const REGISTRY_KEY: &str = r"Software\fi-focus";
const VALUE: &str = "RunInTray";

/// State the tray window procedure needs. It stays on the UI thread because
/// Winit created the window there and Slint state may only be touched there.
#[derive(Clone, Copy)]
struct Context {
    window: HWND,
    previous: Option<WindowProcedure>,
}

thread_local! {
    static CONTEXT: Cell<Option<Context>> = const { Cell::new(None) };
    static APP: RefCell<Option<slint::Weak<crate::App>>> = const { RefCell::new(None) };
    static ICON: Cell<(HICON, bool)> = const { Cell::new((ptr::null_mut(), false)) };
    static VISIBLE: Cell<bool> = const { Cell::new(false) };
}

type WindowProcedure = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

unsafe extern "system" fn window_procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let mut handled = false;
    if message == MESSAGE {
        match lparam as u32 {
            WM_LBUTTONUP | WM_LBUTTONDBLCLK => handled = restore(),
            WM_RBUTTONUP => handled = menu(),
            _ => {}
        }
    } else if message == WM_COMMAND && lparam == 0 && (wparam & 0xffff) == QUIT {
        handled = quit();
    } else if message == WM_CLOSE {
        // Closing natively (title bar, Alt+F4) must follow the same toggle as
        // Tutup instead of letting Winit destroy the window.
        handled = close();
    }
    if handled {
        return 0;
    }
    match CONTEXT.with(Cell::get) {
        Some(context) if context.previous.is_some() => unsafe { CallWindowProcW(context.previous, window, message, wparam, lparam) },
        _ => 0,
    }
}

// Bring the hidden window back and drop the icon while it is visible.
fn restore() -> bool {
    let Some(window) = CONTEXT.with(Cell::get).map(|context| context.window) else { return false; };
    // Showing the native window keeps Winit's state; Slint never hid it.
    unsafe {
        if IsIconic(window) != 0 { ShowWindow(window, SW_RESTORE); }
        ShowWindow(window, SW_SHOW);
        SetForegroundWindow(window);
    }
    true
}

fn menu() -> bool {
    let Some(window) = CONTEXT.with(Cell::get).map(|context| context.window) else { return false; };
    unsafe {
        let handle = CreatePopupMenu();
        if handle.is_null() {
            return false;
        }
        AppendMenuW(handle, MF_STRING, OPEN, wide("Buka fi-focus").as_ptr());
        AppendMenuW(handle, MF_SEPARATOR, 0, ptr::null());
        AppendMenuW(handle, MF_STRING, QUIT, wide("Keluar").as_ptr());
        let mut point = Default::default();
        let _ = GetCursorPos(&mut point);
        // A foreground window is required for the menu to close when the user
        // clicks elsewhere, and the null message dismisses it afterwards.
        SetForegroundWindow(window);
        let command = TrackPopupMenu(handle, TPM_RETURNCMD | TPM_RIGHTBUTTON, point.x, point.y, 0, window, ptr::null());
        let _ = PostMessageW(window, WM_NULL, 0, 0);
        let _ = DestroyMenu(handle);
        match command as usize {
            OPEN => { let _ = restore(); }
            QUIT => { let _ = quit(); }
            _ => {}
        }
    }
    true
}

/// Hide the window while the process keeps running. Native visibility keeps
/// Winit's window alive; hiding through Slint would destroy it and end the loop.
pub fn hide() {
    if let Some(window) = CONTEXT.with(Cell::get).map(|context| context.window) {
        unsafe { ShowWindow(window, SW_HIDE); }
    }
}

fn close() -> bool {
    if !run_in_tray() {
        return false;
    }
    hide();
    true
}

fn quit() -> bool {
    let _ = slint::quit_event_loop();
    true
}

fn wide(text: &str) -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() }

fn icon_data() -> NOTIFYICONDATAW {
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    if let Some(window) = CONTEXT.with(Cell::get).map(|context| context.window) {
        data.hWnd = window;
        data.uID = UID;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = MESSAGE;
        data.hIcon = ICON.with(Cell::get).0;
    }
    for (slot, unit) in data.szTip.iter_mut().zip("fi-focus".encode_utf16()) {
        *slot = unit;
    }
    data
}

// The executable icon at the taskbar's small-icon size, or the shared system
// icon if the executable has none; only an extracted icon is ours to destroy.
fn app_icon(window: HWND) -> (HICON, bool) {
    unsafe {
        let module = GetModuleHandleW(ptr::null());
        if !module.is_null() {
            let base = GetSystemMetrics(SM_CXSMICON).max(16) as u32;
            let size = base * GetDpiForWindow(window).max(96) / 96;
            let icon = ExtractIconW(module, ptr::null(), size);
            if !icon.is_null() {
                return (icon, true);
            }
        }
        (LoadIconW(ptr::null_mut(), IDI_APPLICATION), false)
    }
}

/// Install the icon and intercept the window procedure once the window exists.
pub fn install(ui: &crate::App) -> Result<(), String> {
    if CONTEXT.with(Cell::get).is_some() {
        return Ok(());
    }
    let handle = ui.window().window_handle();
    let Ok(handle) = handle.window_handle() else { return Err("Jendela belum siap".into()); };
    let raw_window_handle::RawWindowHandle::Win32(raw) = handle.as_raw() else {
        return Err("Handle jendela Windows tidak tersedia".into());
    };
    let window = raw.hwnd.get() as HWND;
    let (icon, owned) = app_icon(window);
    if icon.is_null() {
        return Err("Ikon tray tidak tersedia".into());
    }
    let current = unsafe { GetWindowLongPtrW(window, GWLP_WNDPROC) };
    let previous = if current == 0 { None } else { Some(unsafe { std::mem::transmute_copy::<isize, unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>(&current) }) };
    CONTEXT.with(|context| context.set(Some(Context { window, previous })));
    APP.with(|app| *app.borrow_mut() = Some(ui.as_weak()));
    ICON.with(|cell| cell.set((icon, owned)));
    let replaced = unsafe { SetWindowLongPtrW(window, GWLP_WNDPROC, window_procedure as *const () as usize as isize) };
    if previous.is_some() && replaced == 0 {
        reset();
        return Err("Tidak dapat memasang ikon tray".into());
    }
    show();
    Ok(())
}

fn add() {
    let mut data = icon_data();
    VISIBLE.with(|visible| visible.set(unsafe { Shell_NotifyIconW(NIM_ADD, &mut data) } != 0));
}

fn remove() {
    if VISIBLE.with(Cell::get) {
        let mut data = icon_data();
        unsafe { Shell_NotifyIconW(NIM_DELETE, &mut data); }
        VISIBLE.with(|visible| visible.set(false));
    }
}

fn reset() {
    remove();
    if let Some(context) = CONTEXT.with(Cell::get) {
        unsafe { SetWindowLongPtrW(context.window, GWLP_WNDPROC, context.previous.map_or(0, |previous| previous as isize)); }
    }
    let (icon, owned) = ICON.with(Cell::get);
    if owned && !icon.is_null() {
        unsafe { DestroyIcon(icon); }
    }
    ICON.with(|cell| cell.set((ptr::null_mut(), false)));
    CONTEXT.with(|context| context.set(None));
    APP.with(|app| app.borrow_mut().take());
}

/// Removes the icon and restores the original window procedure on drop, so the
/// process neither leaves a ghost icon nor keeps an intercepted window behind.
pub struct Cleanup(());

impl Drop for Cleanup {
    fn drop(&mut self) { reset(); }
}

pub fn guard() -> Cleanup { Cleanup(()) }

/// Add the icon if it is not in the notification area yet.
pub fn show() {
    if !VISIBLE.with(Cell::get) {
        add();
    }
}

/// The remembered toggle value; a missing value keeps the direct-close default.
pub fn run_in_tray() -> bool {
    let name = wide(VALUE);
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, wide(REGISTRY_KEY).as_ptr(), name.as_ptr(), RRF_RT_REG_DWORD,
            ptr::null_mut(), &mut value as *mut u32 as *mut _, &mut size)
    };
    status == 0 && value != 0
}

pub fn remember(run_in_tray: bool) {
    let value = u32::from(run_in_tray);
    unsafe {
        RegSetKeyValueW(HKEY_CURRENT_USER, wide(REGISTRY_KEY).as_ptr(), wide(VALUE).as_ptr(),
            REG_DWORD, &value as *const u32 as *const _, std::mem::size_of::<u32>() as u32);
    }
}
