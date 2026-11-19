#[cfg(target_os = "android")]
pub fn open_url(url: &str, preferred: &str) -> Result<String, String> {
    use jni::objects::{JObject, JValue};
    use jni::JNIEnv;

    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as *mut jni::sys::JavaVM) }.map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread_permanently().map_err(|e| e.to_string())?;
    let activity = unsafe { JObject::from_raw(ctx.context() as jni::sys::jobject) };

    fn try_start(env: &mut JNIEnv<'_>, activity: &JObject<'_>, url: &str, pkg: Option<&str>) -> Result<(), String> {
        let url_j = env.new_string(url).map_err(|e| e.to_string())?;
        let url_obj: JObject = url_j.into();
        let uri = env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&url_obj)]).map_err(|e| e.to_string())?.l().map_err(|e| e.to_string())?;
        let action = env.new_string("android.intent.action.VIEW").map_err(|e| e.to_string())?;
        let action_obj: JObject = action.into();
        let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;Landroid/net/Uri;)V", &[JValue::Object(&action_obj), JValue::Object(&uri)]).map_err(|e| e.to_string())?;
        if let Some(p) = pkg {
            let p_j = env.new_string(p).map_err(|e| e.to_string())?;
            let p_obj: JObject = p_j.into();
            env.call_method(&intent, "setPackage", "(Ljava/lang/String;)Landroid/content/Intent;", &[JValue::Object(&p_obj)]).map_err(|e| e.to_string())?;
        }
        let res = env.call_method(activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)]);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
            return Err("activity not found".into());
        }
        res.map_err(|e| e.to_string())?;
        Ok(())
    }

    if preferred != "default" {
        let mut packages: Vec<&str> = Vec::new();
        if !preferred.is_empty() && preferred != "auto" {
            packages.push(preferred);
        }
        for p in ["com.brave.browser", "com.brave.browser_beta", "com.brave.browser_nightly"] {
            if !packages.contains(&p) {
                packages.push(p);
            }
        }
        for p in packages {
            if try_start(&mut env, &activity, url, Some(p)).is_ok() {
                return Ok(p.to_string());
            }
        }
    }
    try_start(&mut env, &activity, url, None)?;
    Ok("default".to_string())
}

#[cfg(not(target_os = "android"))]
pub fn open_url(url: &str, _preferred: &str) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    let res = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let res = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let res = std::process::Command::new("xdg-open").arg(url).spawn();
    res.map_err(|e| e.to_string())?;
    Ok("default".to_string())
}
