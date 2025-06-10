use rusths::callback::{CallbackWrapper};
use std::ffi::CString;
use libloading::{Library, Symbol};

fn main() {
    // 加载 DLL
    let lib = unsafe { Library::new("path/to/your/dll.dll").unwrap() };

    // 假设 DLL 有一个函数，接受回调函数和用户数据作为参数
    type SetCallbackFn = unsafe extern "C" fn(
        callback: extern "C" fn(*const std::os::raw::c_char, *mut std::ffi::c_void),
        user_data: *mut std::ffi::c_void,
    );

    // 创建回调处理器
    let callback = CallbackWrapper::new(|data| {
        if !data.is_null() {
            unsafe {
                if let Ok(s) = std::ffi::CStr::from_ptr(data).to_str() {
                    println!("收到数据: {}", s);
                }
            }
        }
    });

    // 获取 DLL 中的函数
    let set_callback: Symbol<SetCallbackFn> = unsafe {
        lib.get(b"SetCallback").unwrap()
    };

    // 调用 DLL 函数，设置回调
    unsafe {
        set_callback(rusths::callback::callback_handler, callback.get_user_data());
    }

    // 回调会在 DLL 中被调用
    // CallbackWrapper 会在离开作用域时自动注销回调
} 