use std::sync::{Arc, atomic::{AtomicU8, Ordering}};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::ComponentHandle;
use windows::{core::HSTRING, Foundation::TypedEventHandler, Media::{
    MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
    SystemMediaTransportControlsButton as Button, SystemMediaTransportControlsButtonPressedEventArgs,
}, Win32::{Foundation::HWND, System::WinRT::ISystemMediaTransportControlsInterop}};

pub struct MediaControls {
    controls: SystemMediaTransportControls,
    token: i64,
    last: Arc<AtomicU8>,
}

// Return a card selection, or zero to pause/stop. Play must not toggle an
// already playing card. Previous/next also start playback from a paused state.
fn selection(button: Button, active: i32, last: u8) -> Option<i32> {
    match button {
        Button::Play if active == 0 => Some(last as i32),
        Button::Pause | Button::Stop => Some(0),
        Button::Next => Some((last % 3 + 1) as i32),
        Button::Previous => Some(((last + 1) % 3 + 1) as i32),
        _ => None,
    }
}

impl MediaControls {
    pub fn new(ui: &crate::App) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let handle = ui.window().window_handle();
        // Slint returns an unavailable/unsupported handle until Winit has
        // materialized the window. The UI timer retries this readiness check.
        let Ok(handle) = handle.window_handle() else { return Ok(None); };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err("Handle jendela Windows tidak tersedia".into());
        };
        let interop: ISystemMediaTransportControlsInterop = windows::core::factory::<SystemMediaTransportControls, _>()?;
        // Slint owns this HWND and outlives the media controls.
        let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(HWND(handle.hwnd.get() as _))? };
        controls.SetIsPlayEnabled(true)?;
        controls.SetIsPauseEnabled(true)?;
        controls.SetIsStopEnabled(true)?;
        controls.SetIsNextEnabled(true)?;
        controls.SetIsPreviousEnabled(true)?;
        controls.SetPlaybackStatus(MediaPlaybackStatus::Stopped)?;
        let updater = controls.DisplayUpdater()?;
        updater.SetType(MediaPlaybackType::Music)?;
        updater.MusicProperties()?.SetArtist(&HSTRING::from("fi-focus"))?;
        updater.MusicProperties()?.SetTitle(&HSTRING::from("White noise"))?;
        updater.Update()?;
        controls.SetIsEnabled(true)?;
        let last = Arc::new(AtomicU8::new(1));
        let event_last = last.clone();
        let weak = ui.as_weak();
        let event_controls = controls.clone();
        let token = controls.ButtonPressed(&TypedEventHandler::<SystemMediaTransportControls, SystemMediaTransportControlsButtonPressedEventArgs>::new(move |_, args| {
            let Some(args) = args.as_ref() else { return Ok(()); };
            let button = args.Button()?;
            let last = event_last.clone();
            let controls = event_controls.clone();
            // SMTC events arrive on a system thread; all playback changes run
            // through the same Slint callback as the cards, including fades.
            let _ = weak.upgrade_in_event_loop(move |ui| {
                if let Some(n) = selection(button, ui.get_active(), last.load(Ordering::Relaxed)) {
                    ui.invoke_select(n);
                    if button == Button::Stop {
                        if let Err(e) = controls.SetPlaybackStatus(MediaPlaybackStatus::Stopped) {
                            eprintln!("Media controls: {e}");
                        }
                    }
                }
            });
            Ok(())
        }))?;
        let media = Self { controls, token, last };
        if ui.get_active() != 0 { media.update(ui.get_active() as u8); }
        Ok(Some(media))
    }

    pub fn update(&self, active: u8) {
        let result = (|| -> windows::core::Result<()> {
            if active != 0 {
                self.last.store(active, Ordering::Relaxed);
                let updater = self.controls.DisplayUpdater()?;
                let title = match active { 2 => "Pink noise", 3 => "Brown noise", _ => "White noise" };
                updater.MusicProperties()?.SetTitle(&HSTRING::from(title))?;
                updater.Update()?;
            }
            self.controls.SetPlaybackStatus(if active == 0 { MediaPlaybackStatus::Paused } else { MediaPlaybackStatus::Playing })
        })();
        if let Err(e) = result { eprintln!("Media controls: {e}"); }
    }
}

impl Drop for MediaControls {
    fn drop(&mut self) {
        let _ = self.controls.RemoveButtonPressed(self.token);
        let _ = self.controls.SetPlaybackStatus(MediaPlaybackStatus::Closed);
        let _ = self.controls.SetIsEnabled(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transport_resumes_and_cycles_from_last_noise() {
        for (last, next, previous) in [(1, 2, 3), (2, 3, 1), (3, 1, 2)] {
            assert_eq!(selection(Button::Play, 0, last), Some(last as i32));
            assert_eq!(selection(Button::Play, last as i32, last), None);
            for active in [0, last as i32] {
                assert_eq!(selection(Button::Pause, active, last), Some(0));
                assert_eq!(selection(Button::Stop, active, last), Some(0));
                assert_eq!(selection(Button::Next, active, last), Some(next));
                assert_eq!(selection(Button::Previous, active, last), Some(previous));
            }
        }
        assert_eq!(selection(Button::FastForward, 0, 1), None);
    }
}
