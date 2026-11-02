use std::sync::{Arc, atomic::{AtomicU8, AtomicBool, Ordering}};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use slint::ComponentHandle;
slint::include_modules!();
fn audio(state: Arc<AtomicU8>, silent: Arc<AtomicBool>, failed: Arc<AtomicBool>) -> Result<cpal::Stream, String> {
    let device = cpal::default_host().default_output_device().ok_or("Output audio tidak tersedia")?;
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    let channels = config.channels() as usize;
    let mut seed = 0x12345678u32; let mut b = [0f32; 7]; let mut brown = 0f32;
    // A 5 Hz leak prevents DC drift while preserving the audible brown slope.
    let brown_pole = (-2.0 * std::f32::consts::PI * 5.0 / config.sample_rate() as f32).exp();
    let brown_input = (1.0 - brown_pole * brown_pole).sqrt();
    let step = 1.0 / (config.sample_rate() as f32 * 0.02);
    let mut mode = 0u8;
    let mut gain = 0f32;
    let mut sample = move || -> f32 {
        let target = state.load(Ordering::Relaxed);
        if target != mode {
            gain = (gain - step).max(0.0);
            if gain == 0.0 { mode = target; }
        } else {
            gain = (gain + step).min(1.0);
        }
        silent.store(mode == 0, Ordering::Relaxed);
        if mode == 0 { return 0.0; }
        seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5;
        let w = (seed as f32 / u32::MAX as f32 * 2.0 - 1.0) / 30.0;
        let value = match mode {
            1 => w,
            2 => { b[0]=0.99886*b[0]+w*0.0555179; b[1]=0.99332*b[1]+w*0.0750759; b[2]=0.969*b[2]+w*0.153852; b[3]=0.8665*b[3]+w*0.3104856; b[4]=0.55*b[4]+w*0.5329522; b[5]= -0.7616*b[5]-w*0.016898; let x=(b.iter().sum::<f32>()+w*0.5362)*0.11; b[6]=w*0.115926; x },
            3 => { brown=brown_pole*brown+brown_input*w; brown }, _ => 0.0
        };
        value * gain
    };
    let err = move |e| { eprintln!("Audio: {e}"); failed.store(true, Ordering::Relaxed); };
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(config.into(), move |data: &mut [f32], _| { for frame in data.chunks_mut(channels) { frame.fill(sample()); } }, err, None),
        cpal::SampleFormat::I16 => device.build_output_stream(config.into(), move |data: &mut [i16], _| { for frame in data.chunks_mut(channels) { frame.fill((sample()*32767.0) as i16); } }, err, None),
        cpal::SampleFormat::U16 => device.build_output_stream(config.into(), move |data: &mut [u16], _| { for frame in data.chunks_mut(channels) { frame.fill(((sample()+1.0)*32767.5) as u16); } }, err, None),
        _ => return Err("Format audio tidak didukung".into()),
    }.map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?; Ok(stream)
}
fn start(minimize: impl Fn() + 'static) {
    let state = Arc::new(AtomicU8::new(0));
    let ui = App::new().unwrap();
    #[cfg(target_os="android")] ui.set_android_layout(true);
    let stream = std::rc::Rc::new(std::cell::RefCell::new(None));
    let silent = Arc::new(AtomicBool::new(false));
    let failed = Arc::new(AtomicBool::new(false));
    let weak = ui.as_weak();
    let selected_stream = stream.clone();
    let selected_state = state.clone();
    let selected_silent = silent.clone();
    let selected_failed = failed.clone();
    ui.on_select(move |n| {
        if !(1..=3).contains(&n) { return; }
        let Some(ui) = weak.upgrade() else { return; };
        if selected_failed.swap(false, Ordering::Relaxed) {
            selected_stream.borrow_mut().take();
            selected_state.store(0, Ordering::Relaxed);
        }
        let old = selected_state.load(Ordering::Relaxed);
        let new = if old == n as u8 { 0 } else { n as u8 };
        selected_state.store(new, Ordering::Relaxed);
        selected_silent.store(false, Ordering::Relaxed);
        if new != 0 && selected_stream.borrow().is_none() {
            match audio(selected_state.clone(), selected_silent.clone(), selected_failed.clone()) {
                Ok(audio_stream) => { *selected_stream.borrow_mut() = Some(audio_stream); }
                Err(e) => {
                    eprintln!("Audio: {e}");
                    selected_state.store(0, Ordering::Relaxed);
                    ui.set_active(0);
                    return;
                }
            }
        }
        ui.set_active(new as i32);
    });
    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(50), move || {
        if failed.swap(false, Ordering::Relaxed) {
            stream.borrow_mut().take();
            state.store(0, Ordering::Relaxed);
            if let Some(ui) = weak.upgrade() {
                ui.set_active(0);
            }
        } else if state.load(Ordering::Relaxed) == 0 && silent.load(Ordering::Relaxed) {
            stream.borrow_mut().take();
        }
    });
    let drag_ui = ui.as_weak();
    ui.on_drag(move |x, y| {
        #[cfg(windows)] if let Some(ui) = drag_ui.upgrade() {
            let pos = ui.window().position(); let scale = ui.window().scale_factor();
            ui.window().set_position(slint::PhysicalPosition::new(pos.x + (x*scale) as i32, pos.y + (y*scale) as i32));
        }
        #[cfg(not(windows))] let _ = (&drag_ui, x, y);
    });
    let minimized_ui = ui.as_weak();
    ui.on_minimize_app(move || {
        #[cfg(windows)] if let Some(ui) = minimized_ui.upgrade() { ui.window().set_minimized(true); }
        #[cfg(not(windows))] let _ = &minimized_ui;
        minimize();
    });
    ui.on_close_app(|| { let _ = slint::quit_event_loop(); });
    #[cfg(windows)]
    {
        ui.set_show_window_border(true);
        use windows_sys::Win32::UI::{HiDpi::GetDpiForSystem, WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN}};
        let size = slint::LogicalSize::new(420., 356.);
        unsafe {
            let physical = size.to_physical(GetDpiForSystem().max(96) as f32 / 96.);
            ui.window().set_size(size);
            ui.window().set_position(slint::PhysicalPosition::new(
                (GetSystemMetrics(SM_CXSCREEN) - physical.width as i32) / 2,
                (GetSystemMetrics(SM_CYSCREEN) - physical.height as i32) / 2,
            ));
        }
    }
    ui.run().unwrap();
}
#[cfg(windows)] pub fn run() { slint::BackendSelector::new().backend_name("winit-skia-software".into()).select().unwrap(); start(|| {  }); }
#[cfg(not(windows))] pub fn run() { start(|| {}); }
#[cfg(target_os="android")]
#[unsafe(no_mangle)] fn android_main(app: slint::android::AndroidApp) { let activity_app = app.clone();
    slint::android::init(app).unwrap();
    start(move || {
        let result = (|| -> jni::errors::Result<()> {
            let vm = unsafe { jni::JavaVM::from_raw(activity_app.vm_as_ptr().cast()) }?;
            let mut env = vm.attach_current_thread()?;
            let activity = unsafe { jni::objects::JObject::from_raw(activity_app.activity_as_ptr().cast()) };
            env.call_method(&activity, "minimizeApp", "()V", &[])?;
            Ok(())
        })();
        if let Err(e) = result { eprintln!("Minimize: {e}"); }
    }); }

