//! Load the bundled native adapter only. Cargo development builds need no Sparkle.
//! All calls are made on egui's main thread; the library lives until process exit.
use std::{
    ffi::{c_char, c_void, CString},
    marker::PhantomData,
    rc::Rc,
};

unsafe extern "C" {
    fn dlopen(path: *const c_char, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}
type Query = unsafe extern "C" fn() -> bool;
type Action = unsafe extern "C" fn();
type Setting = unsafe extern "C" fn(bool);

pub struct Updater {
    can_check: Query,
    check: Action,
    automatic: Query,
    set_automatic: Setting,
    set_busy: Setting,
    installing: Query,
    // Native Cocoa objects must never move to a worker thread.
    _main_thread: PhantomData<Rc<()>>,
}
impl Updater {
    pub fn load() -> Option<Self> {
        let executable = std::env::current_exe().ok()?;
        let macos = executable.parent()?;
        if macos.file_name()? != "MacOS" {
            return None;
        }
        let library = macos.parent()?.join("Frameworks/libWhooshUpdater.dylib");
        let path = CString::new(library.to_str()?).ok()?;
        // SAFETY: absolute path within our signed bundle. Symbols implement the C
        // ABI below, and the handle is deliberately kept loaded for Cocoa callbacks.
        unsafe {
            let handle = dlopen(path.as_ptr(), 2); // RTLD_NOW | RTLD_LOCAL
            if handle.is_null() {
                return None;
            }
            macro_rules! symbol {
                ($name:literal, $kind:ty) => {{
                    let pointer = dlsym(handle, concat!($name, "\0").as_ptr().cast());
                    if pointer.is_null() {
                        return None;
                    }
                    std::mem::transmute::<*mut c_void, $kind>(pointer)
                }};
            }
            let start = symbol!("wb_updater_start", Query);
            let result = Self {
                can_check: symbol!("wb_updater_can_check", Query),
                check: symbol!("wb_updater_check", Action),
                automatic: symbol!("wb_updater_automatic", Query),
                set_automatic: symbol!("wb_updater_set_automatic", Setting),
                set_busy: symbol!("wb_updater_set_busy", Setting),
                installing: symbol!("wb_updater_installing", Query),
                _main_thread: PhantomData,
            };
            start().then_some(result)
        }
    }
    pub fn can_check(&self) -> bool {
        unsafe { (self.can_check)() }
    }
    pub fn check(&self) {
        unsafe { (self.check)() }
    }
    pub fn automatic(&self) -> bool {
        unsafe { (self.automatic)() }
    }
    pub fn set_automatic(&self, enabled: bool) {
        unsafe { (self.set_automatic)(enabled) }
    }
    pub fn set_busy(&self, busy: bool) {
        unsafe { (self.set_busy)(busy) }
    }
    pub fn installing(&self) -> bool {
        unsafe { (self.installing)() }
    }
}
