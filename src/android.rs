//! C entrypoint loaded by GTK's Android glue from Neo's cdylib.

use std::os::raw::{c_char, c_int};

use jni::objects::{JObject, JValue};
use relm4::gtk;
use relm4::gtk::glib::translate::ToGlibPtr;
use relm4::gtk::prelude::*;

unsafe extern "C" {
    fn gdk_android_display_get_env(display: *mut std::ffi::c_void) -> *mut jni::sys::JNIEnv;
    fn gdk_android_toplevel_get_activity(
        surface: *mut std::ffi::c_void,
    ) -> jni::sys::jobject;
    fn __android_log_write(
        priority: std::ffi::c_int,
        tag: *const std::ffi::c_char,
        text: *const std::ffi::c_char,
    ) -> std::ffi::c_int;
}

pub fn log(message: &str) {
    let Ok(message) = std::ffi::CString::new(message) else {
        return;
    };
    unsafe {
        __android_log_write(
            4,
            c"Neo".as_ptr(),
            message.as_ptr(),
        );
    }
}

pub fn open_uri(uri: &str, root: &gtk::Box) -> Result<(), String> {
    log(&format!("opening external URI: {uri}"));
    let display = gtk::gdk::Display::default().ok_or("Android display is unavailable")?;
    let native = root
        .native()
        .ok_or("Neo's Android window is unavailable")?;
    let surface = native
        .surface()
        .ok_or("Neo's Android surface is unavailable")?;
    let activity = unsafe {
        gdk_android_toplevel_get_activity(
            <gtk::gdk::Surface as ToGlibPtr<
                '_,
                *mut gtk::gdk::ffi::GdkSurface,
            >>::to_glib_none(&surface)
            .0
            .cast::<std::ffi::c_void>(),
        )
    };
    if activity.is_null() {
        return Err("GTK returned no Android activity".into());
    }
    let raw_env = unsafe {
        gdk_android_display_get_env(
            <gtk::gdk::Display as ToGlibPtr<
                '_,
                *mut gtk::gdk::ffi::GdkDisplay,
            >>::to_glib_none(&display)
            .0
            .cast::<std::ffi::c_void>(),
        )
    };
    let mut env =
        unsafe { jni::JNIEnv::from_raw(raw_env) }.map_err(|error| error.to_string())?;
    let result = (|| -> jni::errors::Result<()> {
        let action = env.new_string("android.intent.action.VIEW")?;
        let uri_text = env.new_string(uri)?;
        let parsed_uri = env
            .call_static_method(
                "android/net/Uri",
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&JObject::from(uri_text))],
            )?
            .l()?;
        let intent = env.new_object(
            "android/content/Intent",
            "(Ljava/lang/String;Landroid/net/Uri;)V",
            &[
                JValue::Object(&JObject::from(action)),
                JValue::Object(&parsed_uri),
            ],
        )?;
        let activity = unsafe { JObject::from_raw(activity) };
        env.call_method(
            activity,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[JValue::Object(&intent)],
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_describe();
            let _ = env.exception_clear();
        }
        return Err(error.to_string());
    }
    log("external URI dispatched to Android activity");
    Ok(())
}

#[no_mangle]
pub extern "C" fn main(
    _argc: c_int,
    _argv: *mut *mut c_char,
    _envp: *mut *mut c_char,
) -> c_int {
    crate::run();
    0
}
