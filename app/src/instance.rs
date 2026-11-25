use std::{io, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM},
    System::Threading::{CreateEventW, CreateMutexW, RegisterWaitForSingleObject, SetEvent, UnregisterWaitEx, INFINITE, WT_EXECUTEDEFAULT},
    UI::WindowsAndMessaging::{AllowSetForegroundWindow, EnumWindows, GetPropW, GetWindowThreadProcessId, PostMessageW, WM_APP},
};

pub const WINDOW_PROPERTY: &str = "fi-focus.MainWindow";
pub const ACTIVATE_WINDOW: u32 = WM_APP + 2;

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) { unsafe { CloseHandle(self.0); } }
}

pub struct Instance {
    _mutex: Handle,
    activation: Handle,
    wait: HANDLE,
}

impl Drop for Instance {
    fn drop(&mut self) {
        if !self.wait.is_null() {
            // Drain callbacks before closing their event.
            unsafe { UnregisterWaitEx(self.wait, INVALID_HANDLE_VALUE); }
        }
    }
}

unsafe extern "system" fn notify_window(window: *mut core::ffi::c_void, _: bool) {
    unsafe { PostMessageW(window, ACTIVATE_WINDOW, 0, 0); }
}

fn wide(text: &str) -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() }

unsafe extern "system" fn allow_foreground(window: HWND, _: LPARAM) -> i32 {
    if !unsafe { GetPropW(window, wide(WINDOW_PROPERTY).as_ptr()) }.is_null() {
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(window, &mut pid);
            AllowSetForegroundWindow(pid);
        }
        return 0;
    }
    1
}

impl Instance {
    pub fn acquire() -> io::Result<Option<Self>> { Self::acquire_named("Local\\fi-focus") }

    pub(crate) fn acquire_named(name: &str) -> io::Result<Option<Self>> {
        // Create the auto-reset event first so a launch during startup is queued.
        let event = unsafe { CreateEventW(ptr::null(), 0, 0, wide(&format!("{name}.Activate")).as_ptr()) };
        if event.is_null() { return Err(io::Error::last_os_error()); }
        let activation = Handle(event);
        let mutex = unsafe { CreateMutexW(ptr::null(), 0, wide(&format!("{name}.Instance")).as_ptr()) };
        if mutex.is_null() { return Err(io::Error::last_os_error()); }
        let existing = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        let instance = Self { _mutex: Handle(mutex), activation, wait: ptr::null_mut() };
        if existing {
            unsafe { EnumWindows(Some(allow_foreground), 0); }
            if unsafe { SetEvent(instance.activation.0) } == 0 { return Err(io::Error::last_os_error()); }
            return Ok(None);
        }
        Ok(Some(instance))
    }

    pub fn watch_activation(&mut self, window: HWND) -> io::Result<()> {
        let mut wait = ptr::null_mut();
        let ok = unsafe { RegisterWaitForSingleObject(&mut wait, self.activation.0,
            Some(notify_window), window, INFINITE, WT_EXECUTEDEFAULT) };
        if ok == 0 { return Err(io::Error::last_os_error()); }
        self.wait = wait;
        Ok(())
    }

    #[cfg(test)]
    fn take_activation_request(&self) -> bool {
        use windows_sys::Win32::{Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject};
        unsafe { WaitForSingleObject(self.activation.0, 0) == WAIT_OBJECT_0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secondary_process_signals_primary_and_lock_is_released() {
        let name = format!("Local\\fi-focus-test-{}", std::process::id());
        let primary = Instance::acquire_named(&name).unwrap().unwrap();
        assert!(!primary.take_activation_request());
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "instance::tests::secondary_process", "--nocapture"])
            .env("FI_FOCUS_TEST_INSTANCE", &name).status().unwrap();
        assert!(status.success());
        assert!(primary.take_activation_request());
        assert!(!primary.take_activation_request());
        drop(primary);
        assert!(Instance::acquire_named(&name).unwrap().is_some());
    }

    #[test]
    #[ignore = "subprocess helper"]
    fn secondary_process() {
        let name = std::env::var("FI_FOCUS_TEST_INSTANCE").unwrap();
        assert!(Instance::acquire_named(&name).unwrap().is_none());
    }
}
