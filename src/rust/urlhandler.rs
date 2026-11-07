//! Reading and setting which app macOS opens a URL scheme with.
//!
//! LaunchServices is the only owner of that choice and has no command line
//! tool, so this calls it directly. Both functions are deprecated in name only:
//! they still route custom schemes on current macOS, and nothing replaces them
//! for a scheme rather than a single URL.

use std::ffi::{c_char, c_void, CStr, CString};

type CFStringRef = *const c_void;

const UTF8: u32 = 0x0800_0100;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(alloc: *const c_void, s: *const c_char, enc: u32) -> CFStringRef;
    fn CFStringGetCString(s: CFStringRef, buf: *mut c_char, len: isize, enc: u32) -> u8;
    fn CFRelease(cf: *const c_void);
}

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn LSCopyDefaultHandlerForURLScheme(scheme: CFStringRef) -> CFStringRef;
    fn LSSetDefaultHandlerForURLScheme(scheme: CFStringRef, bundle_id: CFStringRef) -> i32;
}

/// An owned CFString, released when dropped.
struct Cf(CFStringRef);

impl Cf {
    fn new(s: &str) -> Option<Cf> {
        let c = CString::new(s).ok()?;
        let r = unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), UTF8) };
        (!r.is_null()).then_some(Cf(r))
    }
}

impl Drop for Cf {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}

/// The bundle identifier that opens `scheme://` links, if one is set.
pub fn default_handler(scheme: &str) -> Option<String> {
    let s = Cf::new(scheme)?;
    let got = unsafe { LSCopyDefaultHandlerForURLScheme(s.0) };
    if got.is_null() {
        return None;
    }
    let got = Cf(got);
    let mut buf = [0 as c_char; 512];
    let ok = unsafe { CFStringGetCString(got.0, buf.as_mut_ptr(), buf.len() as isize, UTF8) };
    if ok == 0 {
        return None;
    }
    let text = unsafe { CStr::from_ptr(buf.as_ptr()) };
    Some(text.to_string_lossy().into_owned())
}

/// Makes `bundle_id` the app that opens `scheme://` links.
pub fn set_default_handler(scheme: &str, bundle_id: &str) -> Result<(), String> {
    let (Some(s), Some(b)) = (Cf::new(scheme), Cf::new(bundle_id)) else {
        return Err("invalid scheme or bundle identifier".into());
    };
    match unsafe { LSSetDefaultHandlerForURLScheme(s.0, b.0) } {
        0 => Ok(()),
        status => Err(format!(
            "LaunchServices refused {scheme}:// for {bundle_id} (OSStatus {status})"
        )),
    }
}
