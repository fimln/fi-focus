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
        HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
        Shell::{Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW},
        WindowsAndMessaging::{
            AppendMenuW, CallWindowProcW, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos,
            GetWindowLongPtrW, HICON, IsIconic, LoadIconW, LoadImageW, MF_SEPARATOR, MF_STRING,
            PostMessageW, RegisterWindowMessageW, RemovePropW, SetForegroundWindow, SetPropW, SetWindowLongPtrW, ShowWindow, TrackPopupMenu,
            GWLP_WNDPROC, IDI_APPLICATION, IMAGE_ICON, SM_CXSMICON, SW_HIDE, SW_RESTORE, SW_SHOW, TPM_RETURNCMD, TPM_RIGHTBUTTON,
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
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
}

type WindowProcedure = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

unsafe extern "system" fn window_procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let mut handled = false;
    if message != 0 && message == TASKBAR_CREATED.with(Cell::get) {
        VISIBLE.with(|visible| visible.set(false));
        show();
        if !VISIBLE.with(Cell::get) { let _ = restore(); }
        handled = true;
    } else if message == MESSAGE {
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

// Keep the icon installed while restoring the same window and audio stream.
pub fn restore() -> bool {
    let Some(window) = CONTEXT.with(Cell::get).map(|context| context.window) else { return false; };
    APP.with(|app| {
        if let Some(ui) = app.borrow().as_ref().and_then(slint::Weak::upgrade) {
            ui.window().set_minimized(false);
        }
    });
    // Showing the native window keeps Winit's state; Slint never hid it.
    unsafe {
        if IsIconic(window) != 0 { ShowWindow(window, SW_RESTORE); }
        ShowWindow(window, SW_SHOW);
        SetForegroundWindow(window);
    }
    APP.with(|app| {
        if let Some(ui) = app.borrow().as_ref().and_then(slint::Weak::upgrade) {
            ui.window().request_redraw();
        }
    });
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
pub fn hide() -> bool {
    show();
    // Never hide the only way back into the app if the shell rejected the icon.
    if !VISIBLE.with(Cell::get) { return false; }
    if let Some(window) = CONTEXT.with(Cell::get).map(|context| context.window) {
        unsafe { ShowWindow(window, SW_HIDE); }
        return true;
    }
    false
}

fn close() -> bool {
    if !run_in_tray() {
        return false;
    }
    let _ = hide();
    true
}

fn quit() -> bool {
    let _ = slint::quit_event_loop();
    true
}

fn wide(text: &str) -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() }

pub fn enable_full_repaint(ui: &crate::App) {
    // Slint 1.18.1's Skia renderer disables partial rendering when a notifier is
    // installed. Full frames avoid lost Windows surface contents after restore.
    // Keep this workaround until upgrading to the fix in slint-ui/slint#13568.
    ui.window().set_rendering_notifier(|_, _| {}).expect("Skia rendering notifier");
}

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
// icon if the executable has none; only a loaded resource icon is ours to destroy.
fn app_icon(window: HWND) -> (HICON, bool) {
    unsafe {
        let module = GetModuleHandleW(ptr::null());
        if !module.is_null() {
            let size = GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForWindow(window).max(96)).max(16);
            // winresource embeds the application icon as resource ID 1.
            let icon = LoadImageW(module, 1usize as *const u16, IMAGE_ICON, size, size, 0) as HICON;
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
    unsafe { SetPropW(window, wide(crate::instance::WINDOW_PROPERTY).as_ptr(), 1usize as _); }
    APP.with(|app| *app.borrow_mut() = Some(ui.as_weak()));
    ICON.with(|cell| cell.set((icon, owned)));
    TASKBAR_CREATED.with(|message| message.set(unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) }));
    let replaced = unsafe { SetWindowLongPtrW(window, GWLP_WNDPROC, window_procedure as *const () as usize as isize) };
    if previous.is_some() && replaced == 0 {
        reset();
        return Err("Tidak dapat memasang ikon tray".into());
    }
    show();
    if !VISIBLE.with(Cell::get) {
        reset();
        return Err("Tidak dapat menampilkan ikon tray".into());
    }
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
        unsafe { RemovePropW(context.window, wide(crate::instance::WINDOW_PROPERTY).as_ptr()); }
        unsafe { SetWindowLongPtrW(context.window, GWLP_WNDPROC, context.previous.map_or(0, |previous| previous as isize)); }
    }
    let (icon, owned) = ICON.with(Cell::get);
    if owned && !icon.is_null() {
        unsafe { DestroyIcon(icon); }
    }
    ICON.with(|cell| cell.set((ptr::null_mut(), false)));
    CONTEXT.with(|context| context.set(None));
    TASKBAR_CREATED.with(|message| message.set(0));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_icon_uses_embedded_application_resource() {
        let (icon, owned) = app_icon(ptr::null_mut());
        assert!(!icon.is_null());
        assert!(owned, "The embedded application icon must load without the generic fallback");
        unsafe { DestroyIcon(icon); }
    }

    #[test]
    fn tray_restore_keeps_window_and_selection() {
        use windows_sys::Win32::{
            Foundation::RECT,
            Graphics::Gdi::{CreateSolidBrush, DeleteObject, FillRect, GetDC, GetPixel, InvalidateRect, ReleaseDC},
            UI::WindowsAndMessaging::{GetClientRect, IsWindowVisible},
        };

        fn pixels(window: HWND, scale: f32) -> Vec<u32> {
            let dc = unsafe { GetDC(window) };
            assert!(!dc.is_null());
            let result = [(10., 10.), (10., 150.), (410., 150.), (10., 345.), (200., 270.)]
                .map(|(x, y)| unsafe { GetPixel(dc, (x * scale) as i32, (y * scale) as i32) }).to_vec();
            unsafe { ReleaseDC(window, dc); }
            assert!(!result.contains(&u32::MAX), "The test window must have readable pixels");
            result
        }

        slint::BackendSelector::new().backend_name("winit-skia-software".into()).select().unwrap();
        let ui = crate::App::new().unwrap();
        enable_full_repaint(&ui);
        ui.set_show_window_border(true);
        ui.set_active(2);
        let name = format!("Local\\fi-focus-tray-test-{}", std::process::id());
        let instance = crate::instance::Instance::acquire_named(&name).unwrap().unwrap();
        let weak = ui.as_weak();
        let timer = slint::Timer::default();
        let mut stage = 0;
        let mut original = ptr::null_mut();
        let mut reference = Vec::new();
        timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(50), move || {
            let ui = weak.upgrade().unwrap();
            if stage == 0 {
                install(&ui).unwrap();
                original = CONTEXT.with(Cell::get).unwrap().window;
            }
            if stage == 10 {
                reference = pixels(original, ui.window().scale_factor());
                assert!(hide());
                assert_eq!(unsafe { IsWindowVisible(original) }, 0);
            } else if stage == 12 {
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--ignored", "--exact", "instance::tests::secondary_process", "--nocapture"])
                    .env("FI_FOCUS_TEST_INSTANCE", &name).status().unwrap();
                assert!(status.success());
                assert!(instance.take_activation_request());
                assert!(restore());
            } else if stage == 14 || stage == 18 || stage == 24 {
                assert_ne!(unsafe { IsWindowVisible(original) }, 0);
                assert_eq!(unsafe { IsIconic(original) }, 0);
                assert_eq!(CONTEXT.with(Cell::get).unwrap().window, original);
                assert_eq!(ui.get_active(), 2);
                assert!(VISIBLE.with(Cell::get));
                assert_eq!(pixels(original, ui.window().scale_factor()), reference, "The entire client must be repainted");
                if stage == 14 {
                    // Simulate client contents lost by Windows, then request an OS
                    // repaint without changing any Slint properties or geometry.
                    unsafe {
                        let dc = GetDC(original);
                        let brush = CreateSolidBrush(0x00ff00ff);
                        let mut rect: RECT = std::mem::zeroed();
                        assert_ne!(GetClientRect(original, &mut rect), 0);
                        FillRect(dc, &rect, brush);
                        DeleteObject(brush);
                        ReleaseDC(original, dc);
                        InvalidateRect(original, ptr::null(), 0);
                    }
                }
            } else if stage == 20 {
                ui.window().set_minimized(true);
                assert!(hide());
            } else if stage == 22 {
                assert!(restore());
            }
            stage += 1;
            if stage == 25 { slint::quit_event_loop().unwrap(); }
        });
        let cleanup = guard();
        ui.run().unwrap();
        drop(cleanup);
        assert!(CONTEXT.with(Cell::get).is_none());
        assert!(!VISIBLE.with(Cell::get));
    }
}
