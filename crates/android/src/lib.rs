use jni::JNIEnv;
use jni::objects::JClass;
use jni::sys::jstring;

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_vpet_mobile_PetBridge_nativeVersion(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let output = env
        .new_string("vpet-mobile 0.0.1 (Rust)")
        .expect("Couldn't create java string!");
    output.into_raw()
}
