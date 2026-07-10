use std::os::raw::c_char;

/// 预留的不透明引擎指针句柄
pub struct KotoclipEngine;

/// 初始化引擎 (预留)
#[no_mangle]
pub extern "C" fn kotoclip_init(_config_json: *const c_char) -> *mut KotoclipEngine {
    std::ptr::null_mut()
}

/// 分析整页文本并返回 JSON 字符串 (预留)
#[no_mangle]
pub extern "C" fn kotoclip_analyze(
    _engine: *mut KotoclipEngine,
    _text: *const c_char,
) -> *mut c_char {
    std::ptr::null_mut()
}

/// 释放动态库分配的字符串内存 (预留)
#[no_mangle]
pub extern "C" fn kotoclip_free_string(_ptr: *mut c_char) {}

/// 销毁并释放引擎实例 (预留)
#[no_mangle]
pub extern "C" fn kotoclip_destroy(_engine: *mut KotoclipEngine) {}
